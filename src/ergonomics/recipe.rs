use crate::context::*;
use crate::entity_channel::*;
use crate::entity_id::*;
use crate::error::*;
use crate::expected_entity_channel::*;

use super::entity_channel_ext::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::sync::*;

#[cfg(any(feature="timer", feature="test-scene"))] use futures_timer::{Delay};
#[cfg(any(feature="timer", feature="test-scene"))] use std::time::{Duration};

///
/// A recipe is used to describe a set of actions sent to one or more entities in a scene, in order.
///
/// This is essentially a simple scripting extension, making it possible to encode fixed sets of steps into
/// a script that can be repeatedly executed (for more complicated scripting, a scripting language should
/// probably be used)
///
/// A recipe is useful in a number of situations, but in particular for testing where it can be used to describe a
/// set of messages and expected responses.
///
#[derive(Clone)]
pub struct Recipe {
    /// The entity ID used for channels generated by this recipe
    entity_id: EntityId,

    /// Each step is a boxed function returning a future
    steps: Vec<Arc<dyn Send + Fn(Arc<SceneContext>) -> BoxFuture<'static, Result<(), RecipeError>>>>,
}

///
/// An intermediate build stage for a recipe step that expects a particular response to be sent to a channel
///
pub struct ExpectingRecipe<TExpectedChannel> {
    /// The recipe that the 'expect' step will be appended to
    recipe: Recipe,

    /// Factory method to generate the expected response channel and a future for when the channel has generated all of its expected responses
    responses: Box<dyn Send + Fn(Arc<SceneContext>) -> (TExpectedChannel, BoxFuture<'static, Result<(), RecipeError>>)>,
}

impl Default for Recipe {
    ///
    /// Creates a default (empty) recipe
    ///
    fn default() -> Recipe {
        Recipe {
            entity_id:  EntityId::new(),
            steps:      vec![]
        }
    }
}

impl Recipe {
    ///
    /// Creates a new empty recipe
    ///
    pub fn new() -> Recipe {
        Self::default()
    }

    ///
    /// Runs this recipe
    ///
    pub async fn run(&self, context: Arc<SceneContext>) -> Result<(), RecipeError> {
        // Run the steps in the recipe, stop if any of them generate an error
        for step in self.steps.iter() {
            step(Arc::clone(&context)).await?;
        }

        Ok(())
    }

    ///
    /// Runs this recipe with a timeout
    ///
    /// Requires the `timer` feature.
    ///
    #[cfg(any(feature="timer", feature="test-scene"))] 
    pub async fn run_with_timeout(&self, context: Arc<SceneContext>, timeout: Duration) -> Result<(), RecipeError> {
        // The timeout future is used to abort the test if it takes too long
        let timeout     = Delay::new(timeout)
            .map(|_| Err(RecipeError::Timeout));

        // Create a future to run the steps
        let steps       = self.steps.clone();
        let run_steps   = async move {
            for step in steps.into_iter() {
                step(Arc::clone(&context)).await?;
            }

            Ok(())
        };

        // Pick whichever of the two futures finishes first
        let run_steps       = run_steps.boxed_local();
        let timeout         = timeout.boxed_local();
        let result          = future::select_all(vec![run_steps, timeout]);
        let (result, _, _)  = result.await;

        result
    }
    
    ///
    /// Adds a new step to the recipe that sends a set of fixed messages to an entity
    ///
    pub fn send_messages<TMessage>(self, target_entity_id: EntityId, messages: impl IntoIterator<Item=TMessage>) -> Recipe
    where
        TMessage: 'static + Clone + Send,
    {
        let our_entity_id   = self.entity_id;
        let mut steps       = self.steps;
        let messages        = messages.into_iter().collect::<Vec<_>>();
        let new_step        = Arc::new(move |context: Arc<SceneContext>| {
            let messages = messages.clone();

            async move {
                // Send to the entity
                let mut channel = context.send_to(target_entity_id)?;

                // Copy the messages one at a time
                for msg in messages.into_iter() {
                    channel.send(msg).await?;
                }

                Ok(())
            }.boxed()
        });

        steps.push(new_step);

        Recipe {
            entity_id:  our_entity_id, 
            steps:      steps 
        }
    }

    ///
    /// Adds a new step to the recipe that sends a set of messages generated by a function to an entity
    ///
    /// This can be used for sending messages that are not `Clone`. For messages that send responses to a channel, see `expect()`
    ///
    pub fn send_generated_messages<TMessageIterator>(self, target_entity_id: EntityId, generate_messages: impl 'static + Send + Fn() -> TMessageIterator) -> Recipe
    where
        TMessageIterator:           'static + IntoIterator,
        TMessageIterator::IntoIter: 'static + Send,
        TMessageIterator::Item:     'static + Send,
    {
        let our_entity_id   = self.entity_id;
        let mut steps       = self.steps;
        let new_step        = Arc::new(move |context: Arc<SceneContext>| {
            let messages = generate_messages().into_iter();

            async move {
                // Send to the entity
                let mut channel = context.send_to(target_entity_id)?;

                // Copy the messages one at a time
                for msg in messages {
                    channel.send(msg).await?;
                }

                Ok(())
            }.boxed()
        });

        steps.push(new_step);

        Recipe {
            entity_id:  our_entity_id, 
            steps:      steps 
        }
    }

    ///
    /// Starts to define a step that expects a specific set of responses to be sent to channel
    ///
    /// A channel that will process the responses is supplied to a factory method
    ///
    pub fn expect<TResponse>(self, responses: impl IntoIterator<Item=TResponse>) -> ExpectingRecipe<BoxedEntityChannel<'static, TResponse>>
    where
        TResponse: 'static + Send + Sync + PartialEq,
    {
        let entity_id = self.entity_id;
        let responses = responses.into_iter().collect::<Vec<_>>();
        let responses = Arc::new(responses);

        ExpectingRecipe {
            recipe:     self,
            responses:  Box::new(move |_context| {
                let (channel, future) = ExpectedEntityChannel::new(entity_id, Arc::clone(&responses));

                (channel.boxed(), future.boxed())
            })
        }
    }

    // TODO: a '.alongside_messages()' and a '.alongside_generated_messages()' function for sending to multiple entities in parallel
    // TODO: also an '.alongside_expect()' function for expecting on another channel in parallel with another entity
    // TODO: some way to describe which part of the recipe failed in the error
}

impl<TExpectedChannel> ExpectingRecipe<TExpectedChannel>
where
    TExpectedChannel: 'static + Send,
{
    ///
    /// Sends the messages that expect this response
    ///
    pub fn after_sending_messages<TMessageIterator>(self, target_entity_id: EntityId, generate_messages: impl 'static + Send + Fn(TExpectedChannel) -> TMessageIterator) -> Recipe 
    where
        TMessageIterator:           'static + IntoIterator,
        TMessageIterator::IntoIter: 'static + Send,
        TMessageIterator::Item:     'static + Send,
    {
        let our_entity_id   = self.recipe.entity_id;
        let mut steps       = self.recipe.steps;
        let responses       = self.responses;

        let new_step        = Arc::new(move |context: Arc<SceneContext>| {
            let (channel, future)   = responses(Arc::clone(&context));
            let messages            = generate_messages(channel).into_iter();

            async move {
                // Send to the entity
                let mut channel = context.send_to(target_entity_id)?;

                // Copy the messages one at a time
                for msg in messages {
                    channel.send(msg).await?;
                }

                // Wait for the expected responses to arrive
                future.await?;

                Ok(())
            }.boxed()
        });

        steps.push(new_step);

        Recipe {
            entity_id:  our_entity_id, 
            steps:      steps 
        }
    }
}

impl<TResponse1> ExpectingRecipe<BoxedEntityChannel<'static, TResponse1>> 
where
    TResponse1: 'static + Send + Sync + PartialEq,
{
    ///
    /// As for `Recipe::expect`, except this will extend the number of channels with expectations to 2 
    ///
    pub fn expect<TResponse2>(self, responses: impl IntoIterator<Item=TResponse2>) -> ExpectingRecipe<(BoxedEntityChannel<'static, TResponse1>, BoxedEntityChannel<'static, TResponse2>)>
    where
        TResponse2: 'static + Send + Sync + PartialEq,
    {
        let recipe              = self.recipe;
        let entity_id           = recipe.entity_id;
        let other_responses     = self.responses;
        let responses           = responses.into_iter().collect::<Vec<_>>();
        let responses           = Arc::new(responses);

        ExpectingRecipe {
            recipe:     recipe,
            responses:  Box::new(move |context| {
                // Request the other channel
                let (other_channel, other_future)   = other_responses(context);

                // Create the this channel
                let (our_channel, our_future)       = ExpectedEntityChannel::new(entity_id, Arc::clone(&responses));

                let future = async move {
                    let all_responses = future::join_all(vec![other_future, our_future.boxed()]).await;
                    all_responses.into_iter()
                        .fold(Ok(()), |a, b| a.or(b))
                };

                ((other_channel, our_channel.boxed()), future.boxed())
            })
        }
    }
}

impl<TResponse1, TResponse2> ExpectingRecipe<(BoxedEntityChannel<'static, TResponse1>, BoxedEntityChannel<'static, TResponse2>)> 
where
    TResponse1: 'static + Send + Sync + PartialEq,
    TResponse2: 'static + Send + Sync + PartialEq,
{
    ///
    /// As for `Recipe::expect`, except this will extend the number of channels with expectations to 2 
    ///
    pub fn expect<TResponse3>(self, responses: impl IntoIterator<Item=TResponse3>) -> ExpectingRecipe<(BoxedEntityChannel<'static, TResponse1>, BoxedEntityChannel<'static, TResponse2>, BoxedEntityChannel<'static, TResponse3>)>
    where
        TResponse3: 'static + Send + Sync + PartialEq,
    {
        let recipe              = self.recipe;
        let entity_id           = recipe.entity_id;
        let other_responses     = self.responses;
        let responses           = responses.into_iter().collect::<Vec<_>>();
        let responses           = Arc::new(responses);

        ExpectingRecipe {
            recipe:     recipe,
            responses:  Box::new(move |context| {
                // Request the other channel
                let ((other_channel1, other_channel2), other_future) = other_responses(context);

                // Create the this channel
                let (our_channel, our_future) = ExpectedEntityChannel::new(entity_id, Arc::clone(&responses));

                let future = async move {
                    let all_responses = future::join_all(vec![other_future, our_future.boxed()]).await;
                    all_responses.into_iter()
                        .fold(Ok(()), |a, b| a.or(b))
                };

                ((other_channel1, other_channel2, our_channel.boxed()), future.boxed())
            })
        }
    }
}

impl<TResponse1, TResponse2, TResponse3> ExpectingRecipe<(BoxedEntityChannel<'static, TResponse1>, BoxedEntityChannel<'static, TResponse2>, BoxedEntityChannel<'static, TResponse3>)> 
where
    TResponse1: 'static + Send + Sync + PartialEq,
    TResponse2: 'static + Send + Sync + PartialEq,
    TResponse3: 'static + Send + Sync + PartialEq,
{
    ///
    /// As for `Recipe::expect`, except this will extend the number of channels with expectations to 2 
    ///
    pub fn expect<TResponse4>(self, responses: impl IntoIterator<Item=TResponse4>) -> ExpectingRecipe<(BoxedEntityChannel<'static, TResponse1>, BoxedEntityChannel<'static, TResponse2>, BoxedEntityChannel<'static, TResponse3>, BoxedEntityChannel<'static, TResponse4>)>
    where
        TResponse4: 'static + Send + Sync + PartialEq,
    {
        let recipe              = self.recipe;
        let entity_id           = recipe.entity_id;
        let other_responses     = self.responses;
        let responses           = responses.into_iter().collect::<Vec<_>>();
        let responses           = Arc::new(responses);

        ExpectingRecipe {
            recipe:     recipe,
            responses:  Box::new(move |context| {
                // Request the other channel
                let ((other_channel1, other_channel2, other_channel3), other_future) = other_responses(context);

                // Create the this channel
                let (our_channel, our_future) = ExpectedEntityChannel::new(entity_id, Arc::clone(&responses));

                let future = async move {
                    let all_responses = future::join_all(vec![other_future, our_future.boxed()]).await;
                    all_responses.into_iter()
                        .fold(Ok(()), |a, b| a.or(b))
                };

                ((other_channel1, other_channel2, other_channel3, our_channel.boxed()), future.boxed())
            })
        }
    }
}

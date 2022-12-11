use crate::entity_channel::*;
use crate::entity_id::*;
use crate::error::*;

use futures::prelude::*;
use futures::future;
use futures::future::{BoxFuture};
use futures::channel::oneshot;

use std::sync::*;

///
/// An entity channel that expects a certain response, and completes a future once that response has been generated
///
pub struct ExpectedEntityChannel<TResponse> {
    /// Entity ID that is considered to own this channel
    entity_id: EntityId,

    /// The expected responses for this channel
    expected: Arc<Vec<TResponse>>,

    /// Current position within the list of expected responses
    current_pos: usize,

    /// Where the result should be sent to
    on_completion: Option<oneshot::Sender<Result<(), RecipeError>>>,
}

impl<TResponse> EntityChannel for ExpectedEntityChannel<TResponse>
where
    TResponse: 'static + Send + Sync + PartialEq,
{
    type Message = TResponse;

    fn entity_id(&self) -> EntityId { self.entity_id }

    fn is_closed(&self) -> bool { self.current_pos >= self.expected.len() }

    fn send(&mut self, message: Self::Message) -> BoxFuture<'static, Result<(), EntityChannelError>> {
        if let Some(expected_next) = self.expected.get(self.current_pos) {
            // Move to the next position in the message
            self.current_pos += 1;

            // Message being sent should match the next entity
            if expected_next == &message {
                // Matches the expected message
                if self.current_pos >= self.expected.len() {
                    // Signal success
                    if let Some(on_completion) = self.on_completion.take() {
                        on_completion.send(Ok(())).ok();
                    }
                }

                future::ready(Ok(())).boxed()
            } else {
                // Does not match the expected message
                if let Some(on_completion) = self.on_completion.take() {
                    // Signal an unexpected response
                    on_completion.send(Err(RecipeError::UnexpectedResponse)).ok();
                }

                // Close the channel
                self.current_pos = self.expected.len();
                future::ready(Err(EntityChannelError::NoLongerListening)).boxed()
            }
        } else {
            // Indicate that this channel is closed
            future::ready(Err(EntityChannelError::NoLongerListening)).boxed()
        }
    }
}

impl<TResponse> ExpectedEntityChannel<TResponse>
where
    TResponse: 'static + Send + Sync + PartialEq,
{
    ///
    /// Creates a new expected entity channel and the future that will signal when it's completed
    ///
    pub fn new(entity_id: EntityId, expected_responses: Arc<Vec<TResponse>>) -> (ExpectedEntityChannel<TResponse>, impl Future<Output=Result<(), RecipeError>>) {
        // Create a channel to send the result
        let (result_sender, result_receiver) = oneshot::channel();

        let channel = ExpectedEntityChannel {
            entity_id:      entity_id,
            expected:       expected_responses,
            current_pos:    0,
            on_completion:  Some(result_sender),
        };

        let map_cancelled = result_receiver.map(|maybe_cancelled| {
            match maybe_cancelled {
                Ok(not_cancelled)   => not_cancelled,
                Err(_)              => Err(RecipeError::ExpectedMoreResponses)
            }
        });

        (channel, map_cancelled)
    }
}

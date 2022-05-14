use crate::entity_id::*;
use crate::error::*;
use crate::message::*;
use crate::entity_channel::*;
use crate::scene::scene_core::*;
use crate::standard_components::*;

use ::desync::*;
use futures::prelude::*;
use futures::task::{Poll};
use futures::channel::oneshot;
use futures::stream;
use futures::stream::{BoxStream};

use std::mem;
use std::sync::*;
use std::any::{TypeId};
use std::cell::{RefCell};

thread_local! {
    pub static CURRENT_CONTEXT: RefCell<Option<Arc<SceneContext>>> = RefCell::new(None);
}

///
/// Used to restore the context after a `with_context` call returns
///
struct DropContext {
    previous_context: Option<Arc<SceneContext>>
}

///
/// The context allows for communication with the current scene
///
pub struct SceneContext {
    /// The entity that's executing code on the current thread, or none for things like default actions
    entity: Option<EntityId>,

    /// The core of the scene that the entity is a part of
    scene_core: Result<Arc<Desync<SceneCore>>, SceneContextError>,
}

impl SceneContext {
    ///
    /// Retrieves the active scene context (or a context error if one is available)
    ///
    pub fn current() -> Arc<SceneContext> {
        let context = CURRENT_CONTEXT
            .try_with(|ctxt| ctxt.borrow().clone());

        match context {
            Ok(Some(context))   => context,
            Ok(None)            => Arc::new(SceneContext::none()),
            Err(err)            => Arc::new(SceneContext::error(err.into())),
        }
    }

    ///
    /// Creates a scene context that means 'no context'
    ///
    fn none() -> SceneContext {
        Self::error(SceneContextError::NoCurrentScene)
    }

    ///
    /// Creates an error scene context
    ///
    fn error(error: SceneContextError) -> SceneContext {
        SceneContext {
            entity:     None,
            scene_core: Err(error),
        }
    }

    ///
    /// Returns a context with no active entity 
    ///
    pub (crate) fn with_no_entity(core: &Arc<Desync<SceneCore>>) -> SceneContext {
        SceneContext {
            entity:     None,
            scene_core: Ok(Arc::clone(&core)),
        }
    }

    ///
    /// Evaluates a function within a particular scene context
    ///
    /// This is typically done automatically when running the runtimes for entities, but this can be used if if's ever necessary to
    /// artificially change contexts (eg: if an entity spawns its own thread, or in an independent runtime)
    ///
    #[inline]
    pub fn with_context<TFn, TResult>(new_context: &Arc<SceneContext>, in_context: TFn) -> Result<TResult, SceneContextError>
    where
        TFn: FnOnce() -> TResult
    {
        let new_context = Arc::clone(new_context);

        // When the function returns, reset the context
        let last_context = DropContext {
            previous_context: CURRENT_CONTEXT.try_with(|ctxt| ctxt.borrow().clone())?,
        };

        // Set the updated context
        CURRENT_CONTEXT.try_with(move |ctxt| *(ctxt.borrow_mut()) = Some(new_context))?;

        // Call the function with the new context
        let result = in_context();

        // Restore the context
        mem::drop(last_context);

        Ok(result)
    }

    ///
    /// Returns the entuty that this context is for
    ///
    pub fn entity(&self) -> Option<EntityId> {
        self.entity
    }

    ///
    /// Specify that entities that can process messages of type `TNewMessage` can also process messages of type `TOriginalMessage`
    ///
    /// That is, if an entity can be addressed using `EntityChannel<Message=TNewMessage>` it will automatically convert from `TOriginalMessage`
    /// so that `EntityChannel<Message=TOriginalMessage>` also works.
    ///
    pub fn convert_message<TOriginalMessage, TNewMessage>(&self) -> Result<(), SceneContextError> 
    where
        TOriginalMessage:   'static + Send,
        TNewMessage:        'static + Send + From<TOriginalMessage>,
    {
        self.scene_core.as_ref()?.future_desync(move |core| async move {
            // Register that one type can be converted to another
            core.convert_message::<TOriginalMessage, TNewMessage>();

            // Send to the entity registry
            if let Ok(mut channel) = core.send_to::<InternalRegistryRequest, ()>(ENTITY_REGISTRY) {
                channel.send_without_waiting(InternalRegistryRequest::ConvertMessage(TypeId::of::<TOriginalMessage>(), TypeId::of::<TNewMessage>())).await.ok();
            }
        }.boxed()).detach();

        Ok(())
    }

    ///
    /// Specify that entities that can return responses of type `TOriginalResponse` can also return messages of type `TNewResponse`
    ///
    /// That is, if an entity can be addressed using `EntityChannel<Response=TOriginalResponse>` it will automatically convert from `TNewResponse`
    /// so that `EntityChannel<Response=TNewResponse>` also works.
    ///
    pub fn convert_response<TOriginalResponse, TNewResponse>(&self) -> Result<(), SceneContextError> 
    where
        TOriginalResponse:  'static + Send + Into<TNewResponse>,
        TNewResponse:       'static + Send,
    {
        self.scene_core.as_ref()?.future_desync(move |core| async move {
            // Register that one type can be converted to another
            core.convert_response::<TOriginalResponse, TNewResponse>();

            // Send to the entity registry
            if let Ok(mut channel) = core.send_to::<InternalRegistryRequest, ()>(ENTITY_REGISTRY) {
                channel.send_without_waiting(InternalRegistryRequest::ConvertResponse(TypeId::of::<TOriginalResponse>(), TypeId::of::<TNewResponse>())).await.ok();
            }
        }.boxed()).detach();

        Ok(())
    }

    ///
    /// Creates a channel to send messages in this context
    ///
    pub fn send_to<TMessage, TResponse>(&self, entity_id: EntityId) -> Result<BoxedEntityChannel<'static, TMessage, TResponse>, EntityChannelError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send, 
    {
        self.scene_core.as_ref()?.sync(|core| {
            core.send_to(entity_id)
        })
    }

    ///
    /// Send a single message to an entity in this context
    ///
    pub async fn send<TMessage, TResponse>(&self, entity_id: EntityId, message: TMessage) -> Result<TResponse, EntityChannelError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send, 
    {
        let mut channel = self.send_to::<TMessage, TResponse>(entity_id)?;
        channel.send(message).await
    }

    ///
    /// Given a channel that produces an empty response, sends a message without waiting for the channel to finish processing it
    ///
    pub async fn send_without_waiting<TMessage>(&self, entity_id: EntityId, message: TMessage) -> Result<(), EntityChannelError>
    where
        TMessage:   'static + Send,
    {
        let mut channel = self.send_to::<TMessage, ()>(entity_id)?;
        channel.send_without_waiting(message).await
    }

    ///
    /// Sends a stream of data to an entity
    ///
    /// This will use the `<TMessage, ()>` interface of the entity to send the data
    ///
    pub fn send_stream<TMessage>(&self, entity_id: EntityId, stream: impl 'static + Send + Stream<Item=TMessage>) -> Result<impl Send + Future<Output=()>, EntityChannelError> 
    where
        TMessage:   'static + Send,
    {
        // Connect to the entity
        let mut channel = self.send_to::<TMessage, ()>(entity_id)?;
        let mut stream  = stream.boxed();

        Ok(async move {
            // Future reads from the stream until it's done
            while let Some(message) = stream.next().await {
                // Send to the channel and wait for it to respond
                let response = channel.send(message).await;

                // Break if the channel responds with an error
                if response.is_err() {
                    break;
                }
            }
        })
    }

    ///
    /// Creates an entity that processes a particular kind of message
    ///
    pub fn create_entity<TMessage, TResponse, TFn, TFnFuture>(&self, entity_id: EntityId, runtime: TFn) -> Result<(), CreateEntityError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
        TFn:        'static + Send + FnOnce(BoxStream<'static, Message<TMessage, TResponse>>) -> TFnFuture,
        TFnFuture:  'static + Send + Future<Output = ()>,
    {
        // Create a SceneContext for the new entity
        let new_context = Arc::new(SceneContext {
            entity:     Some(entity_id),
            scene_core: Ok(Arc::clone(self.scene_core.as_ref()?)),
        });

        // Request that the core create the entity
        self.scene_core.as_ref()?.sync(move |core| {
            core.create_entity(new_context, runtime)
        })
    }

    ///
    /// Creates an entity that processes a stream of messages which receive empty responses
    ///
    pub fn create_stream_entity<TMessage, TFn, TFnFuture>(&self, entity_id: EntityId, runtime: TFn) -> Result<(), CreateEntityError>
    where
        TMessage:   'static + Send,
        TFn:        'static + Send + FnOnce(BoxStream<'static, TMessage>) -> TFnFuture,
        TFnFuture:  'static + Send + Future<Output = ()>,
     {
        self.create_entity(entity_id, move |msgs| async {
            let mut last_response: Option<oneshot::Sender<()>>   = None;
            let mut msgs            = msgs;
            let msgs                = stream::poll_fn(move |ctxt| {
                // Respond to the last message before generating the next one (errors are ignored)
                if let Some(last_response) = last_response.take() {
                    last_response.send(()).ok();
                }

                // Retrieve the next message
                match msgs.poll_next_unpin(ctxt) {
                    Poll::Pending           => Poll::Pending,
                    Poll::Ready(None)       => Poll::Ready(None),
                    Poll::Ready(Some(msg))  => {
                        let msg: Message<TMessage, ()> = msg;
                        let (message, response) = msg.take();

                        last_response           = Some(response);
                        Poll::Ready(Some(message))
                    }
                }
            });

            let runtime = runtime(msgs.boxed());

            runtime.await
        })
    }

    ///
    /// Called when an entity in this context has finished
    ///
    pub (crate) fn finish_entity<TMessage, TResponse>(&self, entity_id: EntityId)
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        self.scene_core.as_ref().unwrap()
            .desync(move |core| core.finish_entity(entity_id));
    }

    ///
    /// Called whenever all of the entities in the scene are waiting for new messages
    ///
    pub (crate) fn send_heartbeat(&self) {
        // TODO: hangs?
        self.scene_core.as_ref().unwrap()
            .future_desync(move |core| async move {
                core.send_heartbeat().await;
            }.boxed())
            .detach();
    }
}

impl Drop for DropContext {
    fn drop(&mut self) {
        let previous_context = self.previous_context.take();
        CURRENT_CONTEXT.try_with(move |ctxt| *(ctxt.borrow_mut()) = previous_context).ok();
    }
}

///
/// Retrieves the entity ID that the current context is executing for
///
pub fn scene_current_entity() -> Option<EntityId> {
    SceneContext::current().entity()
}

///
/// Creates a channel for sending messages to a entity (in the current context)
///
pub fn scene_send_to<TMessage, TResponse>(entity_id: EntityId) -> Result<BoxedEntityChannel<'static, TMessage, TResponse>, EntityChannelError>
where
    TMessage:   'static + Send,
    TResponse:  'static + Send, 
{
    SceneContext::current().send_to(entity_id)
}

///
/// Sends a single message to a entity and reads the response
///
pub async fn scene_send<TMessage, TResponse>(entity_id: EntityId, message: TMessage) -> Result<TResponse, EntityChannelError>
where
    TMessage:   'static + Send,
    TResponse:  'static + Send, 
{
    SceneContext::current().send(entity_id, message).await
}

///
/// Sends a stream of data to an entity
///
/// This will use the `<TMessage, ()>` interface of the entity to send the data
///
pub fn scene_send_stream<TMessage>(entity_id: EntityId, stream: impl 'static + Send + Stream<Item=TMessage>) -> Result<impl Send + Future<Output=()>, EntityChannelError> 
where
    TMessage:   'static + Send,
{
    SceneContext::current().send_stream(entity_id, stream)
}

///
/// Creates a new entity in the current scene
///
pub fn scene_create_entity<TMessage, TResponse, TFn, TFnFuture>(entity_id: EntityId, runtime: TFn) -> Result<(), CreateEntityError>
where
    TMessage:   'static + Send,
    TResponse:  'static + Send,
    TFn:        'static + Send + FnOnce(BoxStream<'static, Message<TMessage, TResponse>>) -> TFnFuture,
    TFnFuture:  'static + Send + Future<Output = ()>,
{
    SceneContext::current().create_entity(entity_id, runtime)
}

/// Creates an entity that processes a stream of messages which receive empty responses
///
pub fn scene_create_stream_entity<TMessage, TFn, TFnFuture>(entity_id: EntityId, runtime: TFn) -> Result<(), CreateEntityError>
where
    TMessage:   'static + Send,
    TFn:        'static + Send + FnOnce(BoxStream<'static, TMessage>) -> TFnFuture,
    TFnFuture:  'static + Send + Future<Output = ()>,
{
    SceneContext::current().create_stream_entity(entity_id, runtime)
}

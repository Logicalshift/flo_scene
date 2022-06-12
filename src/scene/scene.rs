use super::scene_core::*;
use super::scene_waker::*;
use crate::entity_id::*;
use crate::context::*;
use crate::error::*;
use crate::message::*;
use crate::entity_channel::*;
use crate::standard_components::*;
use crate::simple_entity_channel::*;
use crate::stream_entity_response_style::*;

use futures::prelude::*;
use futures::future;
use futures::stream::{BoxStream};
use futures::channel::oneshot;
use futures::task;
use futures::task::{Poll};
use ::desync::*;

use std::mem;
use std::sync::*;

///
/// A scene encapsulates a set of entities and provides a runtime for them
///
pub struct Scene {
    /// The shared state for all entities in this scene
    core: Arc<Desync<SceneCore>>,
}

impl Default for Scene {
    ///
    /// Creates a scene with the default set of 'well-known' entities
    ///
    fn default() -> Scene {
        // Create an empty scene
        let scene   = Scene::empty();
        let context = scene.context();

        // Add the standard components
        create_entity_registry_entity(&context).unwrap();
        create_heartbeat_entity(&context).unwrap();

        #[cfg(feature="timer")]
        create_timer_entity(TIMER, &context).unwrap();

        #[cfg(feature="properties")]
        create_properties_entity(PROPERTIES, &context).unwrap();

        scene
    }
}

impl Scene {
    ///
    /// Creates a new scene with no entities defined
    ///
    pub fn empty() -> Scene {
        let core    = SceneCore::default();
        let core    = Arc::new(Desync::new(core));

        Scene {
            core
        }
    }

    ///
    /// Returns the context for this scene
    ///
    pub fn context(&self) -> Arc<SceneContext> {
        Arc::new(SceneContext::with_no_entity(&self.core))
    }

    ///
    /// Creates a channel to send messages in this context
    ///
    pub fn send_to<TMessage, TResponse>(&self, entity_id: EntityId) -> Result<impl EntityChannel<Message=TMessage, Response=TResponse>, EntityChannelError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send, 
    {
        SceneContext::with_no_entity(&self.core).send_to(entity_id)
    }

    ///
    /// Creates an entity that processes a particular kind of message
    ///
    pub fn create_entity<TMessage, TResponse, TFn, TFnFuture>(&self, entity_id: EntityId, runtime: TFn) -> Result<SimpleEntityChannel<TMessage, TResponse>, CreateEntityError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
        TFn:        'static + Send + FnOnce(Arc<SceneContext>, BoxStream<'static, Message<TMessage, TResponse>>) -> TFnFuture,
        TFnFuture:  'static + Send + Future<Output = ()>,
    {
        SceneContext::with_no_entity(&self.core).create_entity(entity_id, runtime)
    }

    ///
    /// Creates an entity that processes a stream of messages which receive empty responses
    ///
    pub fn create_stream_entity<TMessage, TFn, TFnFuture>(&self, entity_id: EntityId, response_style: StreamEntityResponseStyle, runtime: TFn) -> Result<SimpleEntityChannel<TMessage, ()>, CreateEntityError>
    where
        TMessage:   'static + Send,
        TFn:        'static + Send + FnOnce(Arc<SceneContext>, BoxStream<'static, TMessage>) -> TFnFuture,
        TFnFuture:  'static + Send + Future<Output = ()>,
    {
        SceneContext::with_no_entity(&self.core).create_stream_entity(entity_id, response_style, runtime)
    }

    ///
    /// Specify that entities that can process messages of type `TNewMessage` can also process messages of type `TOriginalMessage`
    ///
    /// That is, if an entity can be addressed using `EntityChannel<Message=TNewMessage>` it will automatically convert from `TOriginalMessage`
    /// so that `EntityChannel<Message=TSourceMessage>` also works.
    ///
    pub fn convert_message<TOriginalMessage, TNewMessage>(&self) -> Result<(), SceneContextError> 
    where
        TOriginalMessage:   'static + Send,
        TNewMessage:        'static + Send + From<TOriginalMessage>,
    {
        SceneContext::with_no_entity(&self.core).convert_message::<TOriginalMessage, TNewMessage>()
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
        SceneContext::with_no_entity(&self.core).convert_response::<TOriginalResponse, TNewResponse>()
    }

    ///
    /// Runs this scene
    ///
    pub async fn run(self) {
        // Prepare state (gets moved into the poll function)
        let mut running_futures = vec![];

        let (sender, receiver)  = oneshot::channel();
        self.core.sync(move |core| {
            core.wake_scene = Some(sender);
        });
        let mut wake_receiver   = receiver;

        // Run the scene
        future::poll_fn::<(), _>(move |context| {
            loop {
                // Drain the waiting futures from the core, and load them into our scheduler
                let waiting_futures     = self.core.sync(|core| {
                    let waiting_futures = mem::take(&mut core.waiting_futures);

                    if !waiting_futures.is_empty() || core.wake_scene.is_none() {
                        let (sender, receiver)  = oneshot::channel();
                        core.wake_scene         = Some(sender);
                        wake_receiver           = receiver;
                    }

                    waiting_futures
                });

                // Each future gets its own waker
                let waiting_futures = waiting_futures.into_iter()
                    .map(|future| {
                        let waker = Arc::new(SceneWaker::from_context(context));
                        Some((waker, future))
                    });
                running_futures.extend(waiting_futures);

                // Run futures until they're all asleep again, or the core wakes us
                loop {
                    let mut is_awake            = false;
                    let mut complete_futures    = false;

                    for maybe_future in running_futures.iter_mut() {
                        if let Some((waker, future)) = maybe_future {
                            // Nothing to do if this future isn't awake yet
                            if !waker.is_awake() {
                                continue;
                            }

                            is_awake                = true;

                            // Poll the future to put it back to sleep
                            waker.go_to_sleep(context);

                            let future_waker        = task::waker(Arc::clone(&waker));
                            let mut future_context  = task::Context::from_waker(&future_waker);

                            match future.poll_unpin(&mut future_context) {
                                Poll::Pending   => { }
                                Poll::Ready(_)  => { 
                                    complete_futures    = true;
                                    *maybe_future       = None;
                                }
                            }
                        } else {
                            complete_futures = true;
                        }
                    }

                    // Tidy up any complete futures
                    if complete_futures {
                        running_futures.retain(|future| future.is_some());
                    }

                    // See if the core has woken us up once the futures are polled
                    if let Poll::Ready(_) = wake_receiver.poll_unpin(context) {
                        // Break out of the inner loop to service the core
                        break;
                    }

                    // Stop running once all of the futures are asleep
                    if !is_awake {
                        // Core is asleep, and all of the internal futures are asleep too
                        return Poll::Pending;
                    }
                }   // Inner loop
            }       // Outer loop
        }).await;
    }
}

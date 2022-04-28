use super::scene_core::*;
use super::scene_waker::*;
use crate::entity_id::*;
use crate::context::*;
use crate::error::*;
use crate::message::*;
use crate::entity_channel::*;

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
        Scene::empty()
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
    /// Creates a channel to send messages in this context
    ///
    pub fn send_to<TMessage, TResponse>(&self, entity_id: EntityId) -> Result<EntityChannel<TMessage, TResponse>, EntityChannelError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send, 
    {
        self.core.sync(|core| {
            core.send_to(entity_id)
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
        // Create a SceneContext for the new component
        let new_context = Arc::new(SceneContext::for_entity(entity_id, Arc::clone(&self.core)));

        // Request that the core create the entity
        self.core.sync(move |core| {
            core.create_entity(new_context, runtime)
        })
    }

    ///
    /// Runs this scene
    ///
    pub async fn run(self) {
        // Prepare state (gets moved into the poll function)
        let mut running_futures = vec![];

        // Run the scene
        future::poll_fn::<(), _>(move |context| {
            loop {
                // Drain the waiting futures from the core, and load them into our scheduler
                let (sender, receiver)  = oneshot::channel();
                let waiting_futures     = self.core.sync(move |core| {
                    let waiting_futures = mem::take(&mut core.waiting_futures);
                    core.wake_scene     = Some(sender);
                    waiting_futures
                });
                let mut wake_receiver   = receiver;

                // Each future gets its own waker
                let waiting_futures = waiting_futures.into_iter()
                    .map(|future| {
                        let waker = Arc::new(SceneWaker::from_context(context));
                        (waker, future)
                    });
                running_futures.extend(waiting_futures);

                // Run futures until they're all asleep again, or the core wakes us
                loop {
                    let mut is_awake            = false;
                    let mut complete_futures    = vec![];

                    for (idx, (waker, future)) in running_futures.iter_mut().enumerate() {
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
                            Poll::Ready(()) => { complete_futures.push(idx); }
                        }
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

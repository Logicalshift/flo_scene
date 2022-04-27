use super::scene_core::*;
use super::scene_waker::*;
use crate::entity_id::*;

use futures::prelude::*;
use futures::future;
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
    /// Runs this scene
    ///
    pub async fn run(self) {
        // Prepare state (gets moved into the poll function)
        let mut running_futures = vec![];
        let mut wake_receiver   = None;

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
                wake_receiver           = Some(receiver);

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
                    if let Some(receiver) = &mut wake_receiver {
                        if let Poll::Ready(_) = receiver.poll_unpin(context) {
                            // Finished with the current wake receiver
                            wake_receiver = None;

                            // Break out of the inner loop to service the core
                            break;
                        }
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

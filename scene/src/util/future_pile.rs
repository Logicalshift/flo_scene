use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{waker, ArcWake, Poll, Waker, Context};

use std::sync::*;
use std::collections::{HashMap, HashSet};

///
/// Core data for a future pile
///
struct FuturePileCore {
    /// The ID to assign to the next future in the pile
    next_id: usize,

    /// The futures that are waiting to be run in the pile (set to None when we take them out to poll them)
    futures: HashMap<usize, Option<BoxFuture<'static, ()>>>,

    /// The futures that have been woken up and need to be polled
    awake_futures: HashSet<usize>,

    /// Number of futures that consider themselves 'busy'
    busy_count: usize,

    /// Waker that is called when the future pile is idle
    when_idle: Option<Waker>,

    /// Waker that is used to wake up the runner whenever a new future is added
    waker: Option<Waker>,
}

/// Value whose lifetime represents a 'busy' period with the future pile. Decreases the 'busy' count when dropped
pub struct FuturePileBusy(Weak<Mutex<FuturePileCore>>);

impl Drop for FuturePileBusy {
    fn drop(&mut self) {
        if let Some(core) = self.0.upgrade() {
            // Decrease the busy count, core will be idle at next poll (we don't need to trigger the waker if this is used in a core future, which is the intention)
            core.lock().unwrap().busy_count -= 1;
        }
    }
}

///
/// A futurepile is a task manager for a set of futures that can be extended, or ended once all of the futures have been finished with
///
/// This object can be used to add new futures to the pile
///
#[derive(Clone)]
pub struct FuturePile {
    /// The core is used to add new futures to the pile (weak because we should start throwing futures away after the runner finishes)
    core: Weak<Mutex<FuturePileCore>>,
}

///
/// Future that runs the futures in a future pile
///
pub struct FuturePileRunner {
    /// The core is used to run futures from the pile
    core: Arc<Mutex<FuturePileCore>>,
}

impl FuturePile {
    ///
    /// Creates a new future pile and the corresponding runner
    ///
    pub fn new() -> (FuturePile, FuturePileRunner) {
        // Create a new core
        let core = FuturePileCore {
            next_id:        0,
            futures:        HashMap::new(),
            awake_futures:  HashSet::new(),
            busy_count:     0,
            when_idle:      None,
            waker:          None,
        };
        let core = Arc::new(Mutex::new(core));

        // Put the core into a futurepile and a runner
        let pile    = FuturePile        { core: Arc::downgrade(&core) };
        let runner  = FuturePileRunner  { core: core };

        (pile, runner)
    }

    ///
    /// Adds a future to the set that this pile will run
    ///
    pub fn add_future(&self, new_future: impl 'static + Send + Future<Output=()>) {
        if let Some(core) = self.core.upgrade() {
            // Add the future to the core
            let waker = {
                let mut core = core.lock().unwrap();

                let future_id = core.next_id;
                core.next_id += 1;

                core.futures.insert(future_id, Some(new_future.boxed()));
                core.awake_futures.insert(future_id);

                core.waker.take()
            };

            // Wake up the runner if it's asleep
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    ///
    /// Marks this pile as 'busy' (so the 'idle()' callback will wait)
    ///
    pub fn make_busy(&self) -> FuturePileBusy {
        if let Some(core) = self.core.upgrade() {
            core.lock().unwrap().busy_count += 1;
        }

        FuturePileBusy(self.core.clone())
    }

    ///
    /// Returns a future that waits until there is nothing left waiting in the pile
    ///
    pub fn idle<'a>(&'a self) -> impl 'a + Send + Future<Output=()> {
        future::poll_fn(|ctxt| {
            if let Some(core) = self.core.upgrade() {
                let mut core = core.lock().unwrap();

                if core.awake_futures.is_empty() && core.busy_count == 0 {
                    // No more futures are awake, so we're idle
                    Poll::Ready(())
                } else {
                    // Wake up this thread when we're ready
                    core.when_idle = Some(ctxt.waker().clone());
                    Poll::Pending
                }
            } else {
                Poll::Ready(())
            }
        })
    }
}

/// The waker wakes up a specific future. We use a weak core so there's no reference loop
struct PileWaker(usize, Weak<Mutex<FuturePileCore>>);

impl ArcWake for PileWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let PileWaker(future_id, core) = &**arc_self;
        let future_id = *future_id;

        // Add the future ID to the list of awake futures
        let waker = if let Some(core) = core.upgrade() {
            let mut core = core.lock().unwrap();

            core.awake_futures.insert(future_id);

            core.waker.take()
        } else {
            None
        };

        // Wake up the runtime to poll this future
        if let Some(waker) = waker {
            waker.wake()
        }
    }
}

impl FuturePileRunner {
    ///
    /// Runs the futures that are added to the pile
    ///
    pub fn run_forever(self) -> impl 'static + Send + Unpin + Future<Output=()> {
        future::poll_fn(move |context| {
            // Store a waker for when new futures are added
            self.core.lock().unwrap().waker = Some(context.waker().clone());

            loop {
                // Fetch the futures that are awake from the core
                let awake_futures = {
                    let mut core            = self.core.lock().unwrap();
                    let core                = &mut *core;
                    let mut awake_futures   = Vec::with_capacity(core.awake_futures.len());

                    // Take the futures that need polling from the core
                    let awake_future_ids    = &mut core.awake_futures;
                    let futures             = &mut core.futures;

                    for future_id in awake_future_ids.drain() {
                        let future = futures.get_mut(&future_id).map(|future| future.take());

                        if let Some(Some(future)) = future {
                            awake_futures.push((future_id, future));
                        }
                    }

                    awake_futures
                };

                // Go to sleep once there are no more awake futures
                if awake_futures.is_empty() {
                    // Wake up anything that's waiting for us to become idle (which might re-enter the core)
                    let idle_waker = self.core.lock().unwrap().when_idle.take();

                    if let Some(idle_waker) = idle_waker {
                        idle_waker.wake();
                    }
                }

                if awake_futures.is_empty() {
                    return Poll::Pending;
                }

                // With the core unlocked again, poll the futures
                let mut remaining_futures = vec![];
                for (future_id, mut future) in awake_futures {
                    let future_waker    = Arc::new(PileWaker(future_id, Arc::downgrade(&self.core)));
                    let future_waker    = waker(future_waker);
                    let mut context     = Context::from_waker(&future_waker);

                    match future.poll_unpin(&mut context) {
                        Poll::Ready(()) => {
                            // Future has finished, so don't add it back to the core
                        }

                        Poll::Pending => {
                            // Future is still running, add it back
                            remaining_futures.push((future_id, future));
                        }
                    }
                }

                // Return the futures that were still running after polling to the core
                {
                    let mut core = self.core.lock().unwrap();

                    for (future_id, future) in remaining_futures {
                        core.futures.insert(future_id, Some(future));
                    }
                }
            }
        })
    }
}
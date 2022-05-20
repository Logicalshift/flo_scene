use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task;
use futures::task::{Poll, Context, ArcWake};

use std::mem;
use std::pin::*;
use std::sync::*;
use std::collections::{HashSet};

///
/// State of a future in the core
///
enum CoreFutureState {
    /// Future not used by anything
    Unused,

    /// Newly created future waiting for polling
    NewFuture(BoxFuture<'static, ()>),

    /// Future that is currently being polled
    Active
}

///
/// The core of the background future (designed to be shared between things that need to add new futures or the wakers for individual
/// futures that are running in the background)
///
pub (crate) struct BackgroundFutureCore {
    /// The futures that are running in the background (None for futures that have terminated)
    futures: Vec<CoreFutureState>,

    /// Set to true if new futures are available in the core
    new_futures: bool,

    /// The futures that are awake (indexes into the futures array)
    awake_futures: HashSet<usize>,

    /// If the background future is waiting to wake up, this is the waker to call
    waker: Option<task::Waker>,

    /// Set to true if the future is cancelled (should terminate when set)
    stopped: bool,
}

///
/// Represents the collection of futures that run in the background of an entity
///
pub (crate) struct BackgroundFuture {
    /// Futures that are available for polling
    futures: Vec<Option<BoxFuture<'static, ()>>>,

    /// The core can be used to add or remove new futures while this background future is running
    core: Arc<Mutex<BackgroundFutureCore>>,
}

struct BackgroundWaker {
    /// The index of the future that will be polled
    future_idx: usize,

    /// The core that owns the waker for this future
    core: Arc<Mutex<BackgroundFutureCore>>,
}

impl BackgroundFuture {
    ///
    /// Creates a new background future and its core
    ///
    pub fn new() -> BackgroundFuture {
        let core = BackgroundFutureCore {
            futures:        vec![],
            new_futures:    false,
            awake_futures:  HashSet::new(),
            waker:          None,
            stopped:        false,
        };

        BackgroundFuture {
            futures:    vec![],
            core:       Arc::new(Mutex::new(core)),
        }
    }

    /// 
    /// Retrieves a reference to the core of this future
    ///
    pub (crate) fn core(&self) -> Arc<Mutex<BackgroundFutureCore>> {
        Arc::clone(&self.core)
    }
}

impl Future for BackgroundFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        let awake_futures = {
            // Some weird rust lifetime stuff: partial borrows won't work in a pin, so we need to create a direct self reference
            // ... then we need partial borrows of two fields within self (core and futures) to perform this operation
            let also_self       = &mut *self;
            let self_futures    = &mut also_self.futures;
            let self_core       = &also_self.core;
            let mut core        = self_core.lock().unwrap();

            // Update the waker so next time something needs to wake us up, poll will be called again
            core.waker = Some(context.waker().clone());

            // Terminate all of the futures if the core is stopped
            if core.stopped {
                // Clear any futures from this item
                self_futures.iter_mut().for_each(|future| *future = None);

                // No more work to do (we don't poll any futures any more)
                return Poll::Ready(());
            }

            if core.new_futures {
                // Extend the futures in this struct so it's the same length as the core
                while self_futures.len() < core.futures.len() {
                    self_futures.push(None);
                }

                // Load futures from the core
                for (idx, future) in core.futures.iter_mut().enumerate() {
                    if let CoreFutureState::NewFuture(_) = future {
                        // Swap the future out of the array (marking it as 'active' so a new future won't get written to this slot)
                        let mut new_future = CoreFutureState::Active;
                        mem::swap(&mut new_future, future);

                        // Load into the list of futures to poll
                        if let CoreFutureState::NewFuture(new_future) = new_future {
                            (*self_futures)[idx] = Some(new_future);
                        }
                    }
                }

                // No more new futures
                core.new_futures = false;
            }

            // Load the list of awake futures from the core
            core.awake_futures.drain().collect::<Vec<_>>()
        };

        // Poll the awake futures
        for awake_idx in awake_futures.into_iter() {
            // Make a context for this future to mark it as awake
            let waker           = Arc::new(BackgroundWaker { future_idx: awake_idx, core: Arc::clone(&self.core) });
            let future_waker    = task::waker(waker);
            let mut context     = task::Context::from_waker(&future_waker);

            if let Some(future) = &mut self.futures[awake_idx] {
                // Poll the future
                match future.poll_unpin(&mut context) {
                    Poll::Pending   => { }
                    Poll::Ready(()) => {
                        // Release the future
                        self.futures[awake_idx] = None;

                        // Mark the future as 'available' in the core
                        self.core.lock().unwrap().futures[awake_idx] = CoreFutureState::Unused;
                    }
                }
            }
        }

        // This future itself never actually finishes
        Poll::Pending
    }
}

impl CoreFutureState {
    pub fn is_unused(&self) -> bool {
        match self {
            CoreFutureState::Unused => true,
            _                       => false,
        }
    }
}

///
/// Functions for a background future core reference
///
pub (crate) trait ArcBackgroundFutureCore {
    ///
    /// Wakes the main future
    ///
    fn wake(&self);

    ///
    /// Wakes up a specific future within this background future
    ///
    fn wake_future(&self, future_idx: usize);

    ///
    /// Adds a future to the core, but doesn't wake the main future
    ///
    fn add_future_without_waking(&self, future: impl 'static + Send + Future<Output=()>);

    ///
    /// Add a future to the list that this core owns
    ///
    fn add_future(&self, future: impl 'static + Send + Future<Output=()>);

    ///
    /// Stop the future from running
    ///
    fn stop(&self);
}

impl ArcWake for BackgroundWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.core.wake_future(arc_self.future_idx);
    }
}

impl ArcBackgroundFutureCore for Arc<Mutex<BackgroundFutureCore>> {
    ///
    /// Wakes the main future
    ///
    fn wake(&self) {
        let waker = { self.lock().unwrap().waker.take() };
        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Wakes up a specific future within this background future
    ///
    fn wake_future(&self, future_idx: usize) {
        let waker = { 
            let mut core = self.lock().unwrap();

            core.awake_futures.insert(future_idx);
            core.waker.take() 
        };

        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Adds a future to the core, but doesn't wake the main future
    ///
    fn add_future_without_waking(&self, future: impl 'static + Send + Future<Output=()>) {
        let mut core = self.lock().unwrap();

        // Setting the new futures flag ensures that the futures will be moved into the polling thread next time it's awoken
        core.new_futures = true;

        // Re-use an existing slot if possible
        for (idx, possible_slot) in core.futures.iter_mut().enumerate() {
            if possible_slot.is_unused() {
                *possible_slot = CoreFutureState::NewFuture(future.boxed());
                core.awake_futures.insert(idx);
                return;
            }
        }

        // Create a new future if there are no existing slots
        let new_slot_idx = core.futures.len();
        core.awake_futures.insert(new_slot_idx);
        core.futures.push(CoreFutureState::NewFuture(future.boxed()));
    }

    ///
    /// Add a future to the list that this core owns
    ///
    fn add_future(&self, future: impl 'static + Send + Future<Output=()>) {
        self.add_future_without_waking(future);
        self.wake();
    }

    ///
    /// Stop the future from running
    ///
    fn stop(&self) {
        let waker = { 
            let mut core = self.lock().unwrap();

            core.stopped = true;
            core.waker.take() 
        };

        if let Some(waker) = waker {
            waker.wake()
        }
    }
}

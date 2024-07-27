use futures::future::{BoxFuture};

use std::thread::*;

///
/// A handle of a process running in a scene
///
/// (A process is just a future, a scene is essentially run as a set of concurrent futures that can be modified as needed)
///
#[derive(Copy, Clone, PartialEq, Eq)]
pub (crate) struct ProcessHandle(pub (super) usize);

///
/// The state of the future that's running a process in a scene
///
pub (crate) enum SceneProcessFuture {
    /// The future is not running and is waiting to start
    Waiting(BoxFuture<'static, ()>),

    /// The future is running on the specified thread
    Running(ThreadId),
}

///
/// Data associated with a process in a scene
///
pub (crate) struct SceneProcess {
    /// The future for this process (can be None while it's being polled by another thread)
    pub (super) future: SceneProcessFuture,

    /// Set to true if this process has been woken up
    pub (super) is_awake: bool,

    /// The threads that should be unparked when the future in this process becomes idle 
    pub (super) unpark_when_waiting: Vec<Thread>,
}

impl SceneProcessFuture {
    ///
    /// True if the future is waiting to run
    ///
    #[inline]
    pub fn is_waiting(&self) -> bool {
        matches!(self, SceneProcessFuture::Waiting(_))
    }

    ///
    /// True if this future is already running on this thread
    ///
    #[inline]
    pub fn is_running_on_this_thread(&self) -> bool {
        use std::thread;

        match self {
            SceneProcessFuture::Running(thread_id)  => *thread_id == thread::current().id(),
            _                                       => false,
        }
    }

    ///
    /// If this process is waiting, marks it as running on the current thread and returns the waiting future
    ///
    /// If the process is not waiting, this will return None
    ///
    #[inline]
    pub fn take(&mut self) -> Option<BoxFuture<'static, ()>> {
        use std::mem;
        use std::thread;

        match self {
            SceneProcessFuture::Waiting(_) => {
                // Swap out the result
                let mut result = SceneProcessFuture::Running(thread::current().id());
                mem::swap(&mut result, self);

                // Convert to an option
                if let SceneProcessFuture::Waiting(future) = result { Some(future) } else { unreachable!() }
            },

            _ => None,
        }
    }
}

impl Drop for SceneProcess {
    fn drop(&mut self) {
        // Wake anything that's waiting for this process when it's stopped
        self.unpark_when_waiting.drain(..)
            .for_each(|thread| thread.unpark());
    }
}
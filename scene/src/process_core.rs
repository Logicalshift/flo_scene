use futures::future::{BoxFuture};

///
/// A handle of a process running in a scene
///
/// (A process is just a future, a scene is essentially run as a set of concurrent futures that can be modified as needed)
///
#[derive(Copy, Clone, PartialEq, Eq)]
pub (crate) struct ProcessHandle(pub (super) usize);

///
/// Data associated with a process in a scene
///
pub (crate) struct SceneProcess {
    /// The future for this process (can be None while it's being polled by another thread)
    pub (super) future: Option<BoxFuture<'static, ()>>,

    /// Set to true if this process has been woken up
    pub (super) is_awake: bool,
}

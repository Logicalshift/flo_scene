use futures::future::{BoxFuture};

///
/// A scene represents a set of running co-programs, creating a larger self-contained piece of
/// software out of a set of smaller pieces of software that communicate via streams.
///
pub trait Scene {
    ///
    /// Runs the programs in this scene
    ///
    fn run_scene(self) -> BoxFuture<'static, ()>;
}

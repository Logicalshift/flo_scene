use crate::scene::*;
use crate::stream_target::*;

///
/// Trait implemented by messages that can be sent via a scene
///
pub trait SceneMessage : Sized + Send + Unpin {
    ///
    /// The default target for this message type
    ///
    /// This is `StreamTarget::Any` by default, so streams will wait to be connected. This can be set to `StreamTarget::None`
    /// to throw away messages, or even to a program ID if messages should be sent to a particular program by default.
    ///
    /// Setting a default message target makes it much easier to start programs that use this message type as there's no
    /// need to specifically set up the connections separately. Ideally aim for every message type to have a default target
    /// and only use the `connect_programs()` function to specify exceptions, avoiding the 'wall o'configuration' problem
    /// commonly encountered when using dependency injection to link together a large program.
    ///
    fn default_target() -> StreamTarget { StreamTarget::Any }

    ///
    /// Sets up this message type in a scene. This can be an opportunity to set up default filters and connections for a
    /// particular message type. This is called the first time that a message is referenced in a scene.
    ///
    fn initialise(scene: &Scene) { let _ = scene; }

    ///
    /// True if input streams for this message type should allow thread stealing by default
    ///
    /// Thread stealing will immediately run a future when a message is queued instead of waiting for the future to be
    /// polled in the main loop.
    ///
    fn allow_thread_stealing_by_default() -> bool { false }
}

impl SceneMessage for () { }
impl SceneMessage for &'static str { }
impl SceneMessage for String { }
impl SceneMessage for char { }
impl SceneMessage for usize { }
impl SceneMessage for isize { }
impl SceneMessage for i8 { }
impl SceneMessage for u8 { }
impl SceneMessage for i16 { }
impl SceneMessage for u16 { }
impl SceneMessage for i32 { }
impl SceneMessage for u32 { }
impl SceneMessage for i64 { }
impl SceneMessage for u64 { }
impl SceneMessage for i128 { }
impl SceneMessage for u128 { }

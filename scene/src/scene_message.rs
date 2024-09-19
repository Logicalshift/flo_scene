use crate::scene::*;
use crate::stream_target::*;

use serde::*;

///
/// Trait implemented by messages that can be sent via a scene
///
/// A basic message type can be declared like this:
///
/// ```
/// # use flo_scene::*;
/// #[derive(Serialize, Deserialize)]
/// struct ExampleMessage { some_value: i64 };
///
/// impl SceneMessage for ExampleMessage { }
/// ```
///
/// Messages are initialised the first time they are encountered in a scene. The `initialise()` function can be used to
/// customise this if needed: for example, to set up the default set of connections that a message should support.
///
/// Scene messages should implement the serde serialization primitives but can return only errors. These types should also
/// return `false` from `serializable()` so that the serialization filters aren't generated. Most messages can use 
/// `#[derive(Serialize, Deserialize)]` to generate the serialization routines.
///
/// An implementation like the following can be used for non-serializable messages:
///
/// ```
/// # use flo_scene::*;
/// use serde::*;
/// use serde::de::{Error as DeError};
/// use serde::ser::{Error as SeError};
///
/// struct ExampleMessage;
/// 
/// impl Serialize for ExampleMessage {
///     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
///     where
///         S: Serializer 
///     {
///         Err(S::Error::custom("ExampleMessage cannot be serialized"))
///     }
/// }
/// 
/// impl<'a> Deserialize<'a> for ExampleMessage {
///     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
///     where
///         D: Deserializer<'a> 
///     {
///         Err(D::Error::custom("RunCommand cannot be serialized"))
///     }
/// }
/// 
/// impl SceneMessage for ExampleMessage
/// where
///     TParameter: Unpin + Send,
///     TResponse:  Unpin + Send
/// {
///     fn serializable() -> bool { false }
/// }
/// ```
///
/// Another approach is to serialize via an intermediate type, which can be used when special treatment is needed for serialization
/// or deserialization. This can look like this:
///
/// ```
/// # use flo_scene::*;
/// use serde::*;
/// # 
/// # struct ExampleMessage { real_number: usize }
///
/// #[derive(Serialize, Deserialize)]
/// struct IntermediateMessage { serialized_number: usize }
/// 
/// // This is a contrived example that serializes a different number
///
/// impl Serialize for ExampleMessage {
///     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
///     where
///         S: Serializer 
///     {
///         IntermediateMessage { serialized_number: self.real_number + 1 }.serialize(serializer)
///     }
/// }
/// 
/// impl<'a> Deserialize<'a> for ExampleMessage {
///     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
///     where
///         D: Deserializer<'a> 
///     {
///         let intermediate = IntermediateMessage::deserialize(deserializer)?;
///         Ok(ExampleMessage { real_number: intermediate.serialized_number - 1 })
///     }
/// }
/// ```
///
pub trait SceneMessage :
    Sized                   + 
    Send                    + 
    Unpin                   +
    Serialize               +
    for<'a> Deserialize<'a> + 
{
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

    ///
    /// True if this message supports serialization
    ///
    /// This is true by default, but can be overridden to return false. Messages that are not serializable do not generate
    /// filters for receiving serialized messages.
    ///
    /// All messages must implement the serialization interfaces, but in order to allow messages that are intended to
    /// only be sent within an application (eg, messages that contain function calls or similar non-serializable values,
    /// this can be overridden to return false)
    ///
    fn serializable() -> bool { true }
}

impl SceneMessage for () { }
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

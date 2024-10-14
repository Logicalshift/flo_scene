use crate::host::filter::*;
use crate::host::scene::*;
use crate::host::serialization::*;
use crate::host::stream_target::*;

use serde::*;

///
/// Trait implemented by messages that can be sent via a scene
///
/// A basic message type can be declared like this:
///
/// ```
/// # use flo_scene::*;
/// # use serde::*;
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
///     fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
///     where
///         S: Serializer 
///     {
///         Err(S::Error::custom("ExampleMessage cannot be serialized"))
///     }
/// }
/// 
/// impl<'a> Deserialize<'a> for ExampleMessage {
///     fn deserialize<D>(_: D) -> Result<Self, D::Error>
///     where
///         D: Deserializer<'a> 
///     {
///         Err(D::Error::custom("RunCommand cannot be serialized"))
///     }
/// }
/// 
/// impl SceneMessage for ExampleMessage {
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
    'static                 +
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
    /// Creates the serializer filter handles for this type
    ///
    /// This is an action performed when this type is initialised. This can be overridden to change the default set of 
    /// serialization filters that are installed for this type. Note that this is called even if serializable returns
    /// false, so this can be a way to provide extra filters for a type. 
    ///
    /// The result of this call is cached, so it's not necessary to cache the filters in any other way. The filters returned
    /// here are always installed as stream source filters, so it's better to use `initialise()` for installing any
    /// filters that aren't directly related to serialization.
    ///
    fn create_serializer_filters() -> Vec<FilterHandle> {
        if Self::serializable() {
            // Create some actual serialization filters
            create_default_serializer_filters::<Self>()
        } else {
            // No filters to create
            vec![]
        }
    }

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

    ///
    /// A string that identifies this message type uniquely when serializing
    ///
    /// An error will occur if two types use the same name in the same process. We use `std::any::type_name()` by default
    /// but this does not have a guaranteed format between Rust versions and may not be unique, so it's strongly recommended 
    /// to override this function to return a specific value.
    ///
    fn message_type_name() -> String { std::any::type_name::<Self>().into() }
}

///
/// Creates the default serializer filters for a scene message
///
pub fn create_default_serializer_filters<TMessage: SceneMessage>() -> Vec<FilterHandle> {
    use std::iter;
    use futures::prelude::*;
    use std::sync::*;

    let filters = iter::empty();

    // Convert to and from JSON messages
    #[cfg(feature="json")]
    let filters = {
        // Create the standard to/from JSON filters
        let to_json     = serialization_function::<TMessage, SerializedMessage<serde_json::Value>>().unwrap();
        let from_json   = serialization_function::<SerializedMessage<serde_json::Value>, TMessage>().unwrap();

        let to_json = FilterHandle::for_filter(move |input_messages| {
            let to_json = Arc::clone(&to_json);

            input_messages.flat_map(move |msg| stream::iter((*to_json)(msg).ok()))
        });

        let from_json = FilterHandle::for_filter(move |input_messages| {
            let from_json = Arc::clone(&from_json);

            input_messages.flat_map(move |msg| stream::iter((*from_json)(msg).ok()))
        });

        filters.chain([to_json, from_json])
    };

    // Convert to and from postcard messages
    #[cfg(any(feature="postcard", target_family="wasm"))]
    let filters = {
        // Create the standard to/from postcard filters
        let to_postcard     = serialization_function::<TMessage, SerializedMessage<Postcard>>().unwrap();
        let from_postcard   = serialization_function::<SerializedMessage<Postcard>, TMessage>().unwrap();

        let to_postcard = FilterHandle::for_filter(move |input_messages| {
            let to_postcard = Arc::clone(&to_postcard);

            input_messages.flat_map(move |msg| stream::iter((*to_postcard)(msg).ok()))
        });

        let from_postcard = FilterHandle::for_filter(move |input_messages| {
            let from_postcard = Arc::clone(&from_postcard);

            input_messages.flat_map(move |msg| stream::iter((*from_postcard)(msg).ok()))
        });

        filters.chain([to_postcard, from_postcard])
    };

    filters.collect()
}

impl SceneMessage for ()        { fn message_type_name() -> String { "()".into() } }
impl SceneMessage for String    { fn message_type_name() -> String { "String".into() } }
impl SceneMessage for char      { fn message_type_name() -> String { "char".into() } }
impl SceneMessage for usize     { fn message_type_name() -> String { "usize".into() } }
impl SceneMessage for isize     { fn message_type_name() -> String { "isize".into() } }
impl SceneMessage for i8        { fn message_type_name() -> String { "i8".into() } }
impl SceneMessage for u8        { fn message_type_name() -> String { "u8".into() } }
impl SceneMessage for i16       { fn message_type_name() -> String { "i16".into() } }
impl SceneMessage for u16       { fn message_type_name() -> String { "u16".into() } }
impl SceneMessage for i32       { fn message_type_name() -> String { "i32".into() } }
impl SceneMessage for u32       { fn message_type_name() -> String { "u32".into() } }
impl SceneMessage for i64       { fn message_type_name() -> String { "i64".into() } }
impl SceneMessage for u64       { fn message_type_name() -> String { "u64".into() } }
impl SceneMessage for i128      { fn message_type_name() -> String { "i128".into() } }
impl SceneMessage for u128      { fn message_type_name() -> String { "u128".into() } }

use crate::errors::*;

use serde::*;

#[cfg(feature="json")]
use serde_json;
use postcard;

use alloc::string::*;
use alloc::vec::*;
use alloc::borrow::{Cow};

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
pub trait SceneGuestMessage :
    'static                 +
    Sized                   + 
    Send                    + 
    Unpin                   +
    Serialize               +
    for<'a> Deserialize<'a> + 
{
    ///
    /// A string that identifies this message type uniquely when serializing
    ///
    /// An error will occur if two types use the same name in the same process. We use `std::any::type_name()` by default
    /// but this does not have a guaranteed format between Rust versions and may not be unique, so it's strongly recommended 
    /// to override this function to return a specific value.
    ///
    fn message_type_name() -> String { core::any::type_name::<Self>().into() }

    ///
    /// With the 'json' feature turned on, converts this message to JSON format
    ///
    #[cfg(feature="json")]
    #[inline]
    fn to_json(self) -> Result<serde_json::Value, SceneSendError<Self>> {
        let serializer = serde_json::value::Serializer;
        self.serialize(serializer)
            .map_err(move |json_error|
                SceneSendError::CannotSerialize(self, format!("{:?}", json_error)))
    }

    ///
    /// With the 'json' feature turned on, creates an instance of this message from a JSON value
    ///
    #[cfg(feature="json")]
    #[inline]
    fn from_json(value: &serde_json::Value) -> Result<Self, SceneSendError<()>> {
        Self::deserialize(value)
            .map_err(move |json_error| SceneSendError::CannotDeserialize((), format!("{:?}", json_error)))
    }

    ///
    /// Converts this message to the serialization format used for guest messages
    ///
    #[inline]
    fn to_guest_message(self, context: &impl SerializationContext) -> Result<Vec<u8>, SceneSendError<Self>> {
        let _ = context;
        postcard::to_stdvec(&self)
            .map_err(move |postcard_error| SceneSendError::CannotSerialize(self, postcard_error.to_string()))
    }

    ///
    /// Converts this message from the serialization format used for guest messages
    ///
    #[inline]
    fn from_guest_message(value: &Vec<u8>, context: &impl SerializationContext) -> Result<Self, SceneSendError<()>> {
        let _ = context;
        postcard::from_bytes(value)
            .map_err(move |postcard_error| SceneSendError::CannotDeserialize((), postcard_error.to_string()))
    }
}

impl SceneGuestMessage for ()                { fn message_type_name() -> String { "()".into() } }
impl SceneGuestMessage for String            { fn message_type_name() -> String { "String".into() } }
impl SceneGuestMessage for Cow<'static, str> { fn message_type_name() -> String { "Cow::str".into() } }
impl SceneGuestMessage for char              { fn message_type_name() -> String { "char".into() } }
impl SceneGuestMessage for usize             { fn message_type_name() -> String { "usize".into() } }
impl SceneGuestMessage for isize             { fn message_type_name() -> String { "isize".into() } }
impl SceneGuestMessage for i8                { fn message_type_name() -> String { "i8".into() } }
impl SceneGuestMessage for u8                { fn message_type_name() -> String { "u8".into() } }
impl SceneGuestMessage for i16               { fn message_type_name() -> String { "i16".into() } }
impl SceneGuestMessage for u16               { fn message_type_name() -> String { "u16".into() } }
impl SceneGuestMessage for i32               { fn message_type_name() -> String { "i32".into() } }
impl SceneGuestMessage for u32               { fn message_type_name() -> String { "u32".into() } }
impl SceneGuestMessage for i64               { fn message_type_name() -> String { "i64".into() } }
impl SceneGuestMessage for u64               { fn message_type_name() -> String { "u64".into() } }
impl SceneGuestMessage for i128              { fn message_type_name() -> String { "i128".into() } }
impl SceneGuestMessage for u128              { fn message_type_name() -> String { "u128".into() } }

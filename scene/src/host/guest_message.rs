use crate::host::scene_message::*;
use crate::host::error::*;
use crate::host::serialization_context::*;

pub use flo_scene_guest::host_types::{SceneGuestMessage};

///
/// SceneGuestMessage is a cut-down version of SceneMessage: implicitly implement SceneMessage for every guest message
///
/// Unlike 
///
impl<T: SceneGuestMessage> SceneMessage for T {
    ///
    /// A string that identifies this message type uniquely when serializing
    ///
    /// An error will occur if two types use the same name in the same process. We use `std::any::type_name()` by default
    /// but this does not have a guaranteed format between Rust versions and may not be unique, so it's strongly recommended 
    /// to override this function to return a specific value.
    ///
    fn message_type_name() -> String { <T as SceneGuestMessage>::message_type_name() }

    ///
    /// With the 'json' feature turned on, converts this message to JSON format
    ///
    #[cfg(feature="json")]
    #[inline]
    fn to_json(self) -> Result<serde_json::Value, SceneSendError<Self>> {
        <T as SceneGuestMessage>::to_json(self)
    }

    ///
    /// With the 'json' feature turned on, creates an instance of this message from a JSON value
    ///
    #[cfg(feature="json")]
    #[inline]
    fn from_json(value: &serde_json::Value) -> Result<Self, SceneSendError<()>> {
        <T as SceneGuestMessage>::from_json(self)
    }

    ///
    /// Converts this message to the serialization format used for guest messages
    ///
    #[cfg(any(feature="postcard", target_family="wasm"))]
    #[inline]
    fn to_guest_message(self, context: &impl SerializationContext) -> Result<Vec<u8>, SceneSendError<Self>> {
        <T as SceneGuestMessage>::to_guest_message(self, context)
    }

    ///
    /// Converts this message from the serialization format used for guest messages
    ///
    #[cfg(any(feature="postcard", target_family="wasm"))]
    #[inline]
    fn from_guest_message(value: &Vec<u8>, context: &impl SerializationContext) -> Result<Self, SceneSendError<()>> {
        <T as SceneGuestMessage>::from_guest_message(value, context)
    }
}

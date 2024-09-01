use crate::scene_message::*;

use serde;
use serde_json;

///
/// A guest scene message is one that can be sent to a 'guest' scene. These messages are serializable, and are the type
/// that can be sent to or from a guest scene from a host scene.
///
pub trait GuestSceneMessage : SceneMessage + serde::Serialize + for<'de> serde::Deserialize<'de> {
}

///
/// The guest message encoder
///
pub trait GuestMessageEncoder {
    /// Encodes a guest message
    fn encode(&self, message: impl GuestSceneMessage) -> Vec<u8>;

    /// Decodes a guest message
    fn decode<TMessage: GuestSceneMessage>(&self, message: Vec<u8>) -> TMessage;
}

///
/// Encoder that encodes/decodes JSON messages
///
/// This is a slow and fairly inefficient way to encode messages, but the results are human-readable, which can aid in
/// debugging or interoperability with other systems.
///
pub struct GuestJsonEncoder;

impl GuestMessageEncoder for GuestJsonEncoder {
    #[inline]
    fn encode(&self, message: impl GuestSceneMessage) -> Vec<u8> {
        serde_json::to_string(&message)
            .unwrap()
            .into()
    }

    #[inline]
    fn decode<TMessage: GuestSceneMessage>(&self, message: Vec<u8>) -> TMessage {
        serde_json::from_slice(&message)
            .unwrap()
    }
}
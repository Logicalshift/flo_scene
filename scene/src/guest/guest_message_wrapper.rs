use crate::host::scene_message::*;
use flo_scene_guest::guest_types::*;

use serde::*;

///
/// Adapter that converts a SceneMessage into a SceneGuestMessage, used when a message is not already a SceneGuestMessage
///
pub struct GuestMessageWrapper<TMessage>(pub TMessage);

impl<TMessage: Serialize> Serialize for GuestMessageWrapper<TMessage> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        self.0.serialize(serializer)
    }
}

impl<'de, TMessage: Deserialize<'de>> Deserialize<'de> for GuestMessageWrapper<TMessage> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        TMessage::deserialize(deserializer)
            .map(|val| GuestMessageWrapper(val))
    }
}

impl<TMessage: SceneMessage> SceneGuestMessage for GuestMessageWrapper<TMessage> {
    #[inline]
    fn message_type_name() -> String { TMessage::message_type_name() }

    #[cfg(feature="json")]
    #[inline]
    fn to_json(self) -> Result<serde_json::Value, flo_scene_guest::errors::SceneSendError<Self>> {
        self.0.to_json()
            .map_err(|err| err.map(|val| GuestMessageWrapper(val)))
    }

    #[cfg(feature="json")]
    #[inline]
    fn from_json(value: &serde_json::Value) -> Result<Self, flo_scene_guest::errors::SceneSendError<()>> {
        TMessage::from_json(value)
            .map(|val| GuestMessageWrapper(val))
    }

    #[inline]
    fn to_guest_message(self, context: &impl flo_scene_guest::util::SerializationContext) -> Result<Vec<u8>, flo_scene_guest::errors::SceneSendError<Self>> {
        self.0.to_guest_message(context)
            .map_err(|err| err.map(|val| GuestMessageWrapper(val)))
    }

    #[inline]
    fn from_guest_message(value: &Vec<u8>, context: &impl flo_scene_guest::util::SerializationContext) -> Result<Self, flo_scene_guest::errors::SceneSendError<()>> {
        TMessage::from_guest_message(value, context)
            .map(|val| GuestMessageWrapper(val))
    }
}

pub trait SceneMessageGuestExt : SceneMessage {
    /// Converts this scene message into a guest message
    fn as_guest_message(self) -> GuestMessageWrapper<Self>;
}

impl<TMessage: SceneMessage> SceneMessageGuestExt for TMessage {
    #[inline]
    fn as_guest_message(self) -> GuestMessageWrapper<Self> { GuestMessageWrapper(self) }
}

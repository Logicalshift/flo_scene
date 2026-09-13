use crate::host::initialisation_context::*;
use crate::host::error::*;
use crate::host::filter::*;
use crate::host::scene_message::*;
use crate::host::serialization_context::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;

use futures::prelude::*;
use serde::*;

///
/// Message accepted by a program that wants to receive either of two different message types
///
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EitherMessage<TLeft, TRight> {
    Left(TLeft),
    Right(TRight),
}

impl<TLeft, TRight> SceneMessage for EitherMessage<TLeft, TRight>
where
    TLeft:  SceneMessage,
    TRight: SceneMessage,
{
    fn default_target() -> StreamTarget { StreamTarget::Any }

    fn initialise(init_context: &impl SceneInitialisationContext) {
        // Left/Right will generate their own initialisation when they're used for the first time
        init_context.connect_programs(StreamSource::Filtered(FilterHandle::for_filter(|left| left.map(|left| Self::Left(left)))), (), StreamId::with_message_type::<TLeft>()).ok();
        init_context.connect_programs(StreamSource::Filtered(FilterHandle::for_filter(|right| right.map(|right| Self::Right(right)))), (), StreamId::with_message_type::<TRight>()).ok();
    }

    fn allow_thread_stealing_by_default() -> bool {
        TLeft::allow_thread_stealing_by_default() && TRight::allow_thread_stealing_by_default()
    }

    fn serializable() -> bool {
        TLeft::serializable() && TRight::serializable()
    }

    fn message_type_name() -> String {
        format!("flo_scene::either::<{}, {}>", TLeft::message_type_name(), TRight::message_type_name())
    }

    #[cfg(feature="json")]
    #[inline]
    fn to_json(self) -> Result<serde_json::Value, SceneSendError<Self>> {
        use serde_json::*;

        match self {
            Self::Left(left) => {
                let left = left.to_json().map_err(|err| err.map(|err| Self::Left(err)))?;
                Ok(json!({
                    "left": left
                }))
            }

            Self::Right(right) => {
                let right = right.to_json().map_err(|err| err.map(|err| Self::Right(err)))?;
                Ok(json!({
                    "right": right
                }))
            }
        }
    }

    #[cfg(feature="json")]
    #[inline]
    fn from_json(value: &serde_json::Value) -> Result<Self, SceneSendError<()>> {
        if let Some(left) = value.get("left") {
            Ok(Self::Left(TLeft::from_json(left)?))
        } else if let Some(right) = value.get("right") {
            Ok(Self::Right(TRight::from_json(right)?))
        } else {
            Err(SceneSendError::CannotDeserialize((), "Incorrect format (needs to be left or right)".into()))
        }
    }

    #[cfg(any(feature="postcard", target_family="wasm"))]
    #[inline]
    fn to_guest_message(self, context: &impl SerializationContext) -> Result<Vec<u8>, SceneSendError<Self>> {
        match self {
            Self::Left(left) => {
                let mut left = left.to_guest_message(context).map_err(|err| err.map(|err| Self::Left(err)))?;
                left.insert(0, 0u8);

                Ok(left)
            }

            Self::Right(right) => {
                let mut right = right.to_guest_message(context).map_err(|err| err.map(|err| Self::Right(err)))?;
                right.insert(0, 1u8);

                Ok(right)
            }
        }
    }

    #[cfg(any(feature="postcard", target_family="wasm"))]
    #[inline]
    fn from_guest_message(value: &[u8], context: &impl SerializationContext) -> Result<Self, SceneSendError<()>> {
        match value.get(0) {
            Some(0u8) => {
                let left = TLeft::from_guest_message(&value[1..], context)?;
                Ok(Self::Left(left))
            }

            Some(1u8) => {
                let right = TRight::from_guest_message(&value[1..], context)?;
                Ok(Self::Right(right))
            }

            _ => {
                Err(SceneSendError::CannotDeserialize((), "Not Left or Right".into()))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(any(feature="json", feature="postcard"))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct MessageA(String);

    #[cfg(any(feature="json", feature="postcard"))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct MessageB(u32);

    #[cfg(any(feature="json", feature="postcard"))]
    impl SceneMessage for MessageA { }
    
    #[cfg(any(feature="json", feature="postcard"))]
    impl SceneMessage for MessageB { }

    #[test]
    #[cfg(feature="json")]
    fn left_to_json() {
        let msg     = EitherMessage::<MessageA, MessageB>::Left(MessageA("Left".into()));
        let json    = msg.to_json().unwrap();

        assert!(json == serde_json::json!({ "left": "Left" }), "{:?}", json);
    }

    #[test]
    #[cfg(feature="json")]
    fn right_to_json() {
        let msg     = EitherMessage::<MessageA, MessageB>::Right(MessageB(42));
        let json    = msg.to_json().unwrap();

        assert!(json == serde_json::json!({ "right": 42 }), "{:?}", json);
    }

    #[test]
    #[cfg(feature="json")]
    fn left_from_json() {
        let json    = serde_json::json!({ "left": "Left" });
        let msg     = EitherMessage::<MessageA, MessageB>::from_json(&json).unwrap();

        assert!(msg == EitherMessage::Left(MessageA("Left".into())), "{:?}", msg);
    }

    #[test]
    #[cfg(feature="json")]
    fn right_from_json() {
        let json    = serde_json::json!({ "right": 42 });
        let msg     = EitherMessage::<MessageA, MessageB>::from_json(&json).unwrap();

        assert!(msg == EitherMessage::Right(MessageB(42)), "{:?}", msg);
    }

    #[test]
    #[cfg(feature="json")]
    fn invalid_json() {
        let json    = serde_json::json!({ "neither": 42 });
        let msg     = EitherMessage::<MessageA, MessageB>::from_json(&json);

        assert!(msg.is_err(), "{:?}", msg);
    }

    #[test]
    #[cfg(feature="json")]
    fn json_round_trip() {
        let left    = EitherMessage::<MessageA, MessageB>::Left(MessageA("Left".into()));
        let right   = EitherMessage::<MessageA, MessageB>::Right(MessageB(42));

        let left_json   = left.clone().to_json().unwrap();
        let right_json  = right.clone().to_json().unwrap();

        assert!(EitherMessage::<MessageA, MessageB>::from_json(&left_json).unwrap() == left);
        assert!(EitherMessage::<MessageA, MessageB>::from_json(&right_json).unwrap() == right);
    }

    #[test]
    #[cfg(feature="postcard")]
    fn left_to_guest_message() {
        let msg     = EitherMessage::<MessageA, MessageB>::Left(MessageA("Left".into()));
        let encoded = msg.to_guest_message(&DisconnectedSerializationContext).unwrap();

        let mut expected = vec![0u8];
        expected.extend(MessageA("Left".into()).to_guest_message(&DisconnectedSerializationContext).unwrap());

        assert!(encoded == expected, "{:?} != {:?}", encoded, expected);
    }

    #[test]
    #[cfg(feature="postcard")]
    fn right_to_guest_message() {
        let msg     = EitherMessage::<MessageA, MessageB>::Right(MessageB(42));
        let encoded = msg.to_guest_message(&DisconnectedSerializationContext).unwrap();

        let mut expected = vec![1u8];
        expected.extend(MessageB(42).to_guest_message(&DisconnectedSerializationContext).unwrap());

        assert!(encoded == expected, "{:?} != {:?}", encoded, expected);
    }

    #[test]
    #[cfg(feature="postcard")]
    fn left_from_guest_message() {
        let mut encoded = vec![0u8];
        encoded.extend(MessageA("Left".into()).to_guest_message(&DisconnectedSerializationContext).unwrap());

        let msg = EitherMessage::<MessageA, MessageB>::from_guest_message(&encoded, &DisconnectedSerializationContext).unwrap();

        assert!(msg == EitherMessage::Left(MessageA("Left".into())), "{:?}", msg);
    }

    #[test]
    #[cfg(feature="postcard")]
    fn right_from_guest_message() {
        let mut encoded = vec![1u8];
        encoded.extend(MessageB(42).to_guest_message(&DisconnectedSerializationContext).unwrap());

        let msg = EitherMessage::<MessageA, MessageB>::from_guest_message(&encoded, &DisconnectedSerializationContext).unwrap();

        assert!(msg == EitherMessage::Right(MessageB(42)), "{:?}", msg);
    }

    #[test]
    #[cfg(feature="postcard")]
    fn from_invalid_guest_message() {
        let empty   = EitherMessage::<MessageA, MessageB>::from_guest_message(&[], &DisconnectedSerializationContext);
        let bad_tag = EitherMessage::<MessageA, MessageB>::from_guest_message(&[2u8, 42u8], &DisconnectedSerializationContext);

        assert!(empty.is_err(), "{:?}", empty);
        assert!(bad_tag.is_err(), "{:?}", bad_tag);
    }

    #[test]
    #[cfg(feature="postcard")]
    fn guest_message_round_trip() {
        let left    = EitherMessage::<MessageA, MessageB>::Left(MessageA("Left".into()));
        let right   = EitherMessage::<MessageA, MessageB>::Right(MessageB(42));

        let left_encoded    = left.clone().to_guest_message(&DisconnectedSerializationContext).unwrap();
        let right_encoded   = right.clone().to_guest_message(&DisconnectedSerializationContext).unwrap();

        assert!(EitherMessage::<MessageA, MessageB>::from_guest_message(&left_encoded, &DisconnectedSerializationContext).unwrap() == left);
        assert!(EitherMessage::<MessageA, MessageB>::from_guest_message(&right_encoded, &DisconnectedSerializationContext).unwrap() == right);
    }
}

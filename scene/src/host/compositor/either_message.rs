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
            Self::from_json(left)
        } else if let Some(right) = value.get("right") {
            Self::from_json(right)
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
                right.insert(1, 0u8);

                Ok(right)
            }
        }
    }

    #[cfg(any(feature="postcard", target_family="wasm"))]
    #[inline]
    fn from_guest_message(value: &Vec<u8>, context: &impl SerializationContext) -> Result<Self, SceneSendError<()>> {
        todo!("Needs value to be a slice, not an &Vec<u8>")
    }
}

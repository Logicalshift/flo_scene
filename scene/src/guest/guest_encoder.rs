use crate::host::error::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::stream_target::*;

use futures::prelude::*;
use serde_json;

///
/// The guest message encoder
///
pub trait GuestMessageEncoder : Send + Sync + Clone {
    /// Encodes a guest message
    fn encode(&self, message: impl SceneMessage) -> Vec<u8>;

    /// Decodes a guest message
    fn decode<TMessage: SceneMessage>(&self, message: Vec<u8>) -> TMessage;

    /// Creates a connection to a host stream
    fn connect(&self, stream_id: StreamId, target: StreamTarget, context: &SceneContext) -> Result<impl Sink<Vec<u8>, Error=SceneSendError<Vec<u8>>>, ConnectionError>;
}

///
/// Encoder that encodes/decodes JSON messages
///
/// This is a slow and fairly inefficient way to encode messages, but the results are human-readable, which can aid in
/// debugging or interoperability with other systems.
///
#[derive(Clone)]
pub struct GuestJsonEncoder;

impl GuestMessageEncoder for GuestJsonEncoder {
    #[inline]
    fn encode(&self, message: impl SceneMessage) -> Vec<u8> {
        serde_json::to_string(&message)
            .unwrap()
            .into()
    }

    #[inline]
    fn decode<TMessage: SceneMessage>(&self, message: Vec<u8>) -> TMessage {
        serde_json::from_slice(&message)
            .unwrap()
    }

    fn connect(&self, stream_id: StreamId, target: StreamTarget, context: &SceneContext) -> Result<impl Sink<Vec<u8>, Error=SceneSendError<Vec<u8>>>, ConnectionError> {
        // TODO: actually connect the stream
        if false {
            Ok(sink::drain().sink_map_err(|_| SceneSendError::TargetProgramEndedBeforeReady))
        } else {
            Err(ConnectionError::TargetNotInScene)
        }
    }
}
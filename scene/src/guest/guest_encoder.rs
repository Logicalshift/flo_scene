use crate::host::error::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::stream_target::*;
use crate::host::serialization::*;

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
    fn connect(&self, stream_id: StreamId, target: StreamTarget, context: &SceneContext) -> Result<impl Send + Unpin + Sink<Vec<u8>, Error=SceneSendError<Vec<u8>>>, ConnectionError>;
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

    fn connect(&self, stream_id: StreamId, target: StreamTarget, context: &SceneContext) -> Result<impl Send + Unpin + Sink<Vec<u8>, Error=SceneSendError<Vec<u8>>>, ConnectionError> {
        // Create the stream target
        let serialized_target = SerializedStreamTarget::from(stream_id);
        let serialized_target = match target {
            StreamTarget::None | StreamTarget::Any  => Ok(serialized_target),
            StreamTarget::Program(program_id)       => todo!("Cannot map a target program to a specific stream ID at the moment"),
            StreamTarget::Filtered(_, _)            => Err(ConnectionError::FilterMappingMissing)
        }?;

        // Send as a JSON stream
        let json_stream = context.send_serialized::<serde_json::Value>(serialized_target)?;

        // Put a JSON parser in front of the stream
        let json_stream = json_stream
            .sink_map_err(|_| SceneSendError::TargetProgramEndedBeforeReady /* TODO */)
            .with(|bytes: Vec<u8>| async move {
                let value = serde_json::from_slice::<serde_json::Value>(&bytes);

                match value {
                    Ok(value)   => Ok(value),
                    Err(_)      => Err(SceneSendError::CannotDeserialize(bytes))
                }
            });

        Ok(Box::pin(json_stream))
    }
}
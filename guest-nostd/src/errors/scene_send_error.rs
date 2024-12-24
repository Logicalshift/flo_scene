use super::connection_error::*;

use serde::*;

use alloc::string::*;

///
/// Error that occurs while sending to a stream
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
pub enum SceneSendError<TMessage> {
    /// A message could not be sent because the connection failed
    CouldNotConnect(ConnectionError),

    /// The target program ended while waiting for it to become ready (or after sending the message but before it could be flushed)
    TargetProgramEndedBeforeReady,

    /// The target stream was closed when the message was sent (eg, because the target program is not listening for input)
    StreamClosed(TMessage),

    /// The target for the stream stopped before the message could be sent (can be treated as the same as StreamClosed)
    TargetProgramEnded(TMessage),

    /// The stream is disconnected, so messages cannot currently be sent to it
    StreamDisconnected(TMessage),

    /// The target program supports thread stealing, but it is already running on the current thread's callstack and can't re-enter
    CannotReEnterTargetProgram,

    /// The target program is waiting for the scene to become idle and its input queue is full
    CannotAcceptMoreInputUntilSceneIsIdle(TMessage),

    /// The message could not be serialized
    CannotSerialize(TMessage, String),

    /// The target cannot deserialize this message to a target type
    CannotDeserialize(TMessage, String),

    /// An error occurred after deserialization (and the original message was lost)
    ErrorAfterDeserialization,

    /// Cannot send/receive a stream or a function because there is no backing connection for the message
    NoConnection(TMessage),
}

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

impl<TMessage> SceneSendError<TMessage> {
    ///
    /// Returns `Some(message)` if this error contains the message that failed to send
    ///
    /// A message might not be returned if the failure occurred after the message was added to the input queue for the
    /// target program. Additionally, no message is provided for failures that occur while waiting for the input stream
    /// to become ready.
    ///
    pub fn message(&self) -> Option<&TMessage> {
        match self {
            SceneSendError::CouldNotConnect(_)                          => None,
            SceneSendError::TargetProgramEndedBeforeReady               => None,
            SceneSendError::StreamClosed(msg)                           => Some(msg),
            SceneSendError::TargetProgramEnded(msg)                     => Some(msg),
            SceneSendError::StreamDisconnected(msg)                     => Some(msg),
            SceneSendError::CannotReEnterTargetProgram                  => None,
            SceneSendError::CannotAcceptMoreInputUntilSceneIsIdle(msg)  => Some(msg),
            SceneSendError::CannotSerialize(msg, _)                     => Some(msg),
            SceneSendError::CannotDeserialize(msg, _)                   => Some(msg),
            SceneSendError::ErrorAfterDeserialization                   => None,
            SceneSendError::NoConnection(msg)                           => Some(msg),
        }
    }

    ///
    /// Returns `Some(message)` if this error contains the message that failed to send. This version extract the message
    /// and discards this object. `message()` will return a reference to the message contained within the object.
    ///
    /// A message might not be returned if the failure occurred after the message was added to the input queue for the
    /// target program. Additionally, no message is provided for failures that occur while waiting for the input stream
    /// to become ready.
    ///
    pub fn to_message(self) -> Option<TMessage> {
        match self {
            SceneSendError::CouldNotConnect(_)                          => None,
            SceneSendError::TargetProgramEndedBeforeReady               => None,
            SceneSendError::StreamClosed(msg)                           => Some(msg),
            SceneSendError::TargetProgramEnded(msg)                     => Some(msg),
            SceneSendError::StreamDisconnected(msg)                     => Some(msg),
            SceneSendError::CannotReEnterTargetProgram                  => None,
            SceneSendError::CannotAcceptMoreInputUntilSceneIsIdle(msg)  => Some(msg),
            SceneSendError::CannotDeserialize(msg, _)                   => Some(msg),
            SceneSendError::CannotSerialize(msg, _)                     => Some(msg),
            SceneSendError::ErrorAfterDeserialization                   => None,
            SceneSendError::NoConnection(msg)                           => Some(msg),
        }
    }

    ///
    /// Maps the content of this error to another message type
    ///
    pub fn map<TTarget>(self, map_fn: impl FnOnce(TMessage) -> TTarget) -> SceneSendError<TTarget> {
        match self {
            SceneSendError::CouldNotConnect(msg)                        => SceneSendError::CouldNotConnect(msg),
            SceneSendError::TargetProgramEndedBeforeReady               => SceneSendError::TargetProgramEndedBeforeReady,
            SceneSendError::StreamClosed(msg)                           => SceneSendError::StreamClosed(map_fn(msg)),
            SceneSendError::TargetProgramEnded(msg)                     => SceneSendError::TargetProgramEnded(map_fn(msg)),
            SceneSendError::StreamDisconnected(msg)                     => SceneSendError::StreamDisconnected(map_fn(msg)),
            SceneSendError::CannotReEnterTargetProgram                  => SceneSendError::CannotReEnterTargetProgram,
            SceneSendError::CannotAcceptMoreInputUntilSceneIsIdle(msg)  => SceneSendError::CannotAcceptMoreInputUntilSceneIsIdle(map_fn(msg)),
            SceneSendError::CannotDeserialize(msg, error)               => SceneSendError::CannotDeserialize(map_fn(msg), error),
            SceneSendError::CannotSerialize(msg, error)                 => SceneSendError::CannotSerialize(map_fn(msg), error),
            SceneSendError::ErrorAfterDeserialization                   => SceneSendError::ErrorAfterDeserialization,
            SceneSendError::NoConnection(msg)                           => SceneSendError::NoConnection(map_fn(msg)),
        }
    }
}

#[cfg(feature="serde_support")] use serde::*;

///
/// The name of the message type that is accepted by a subprogram
///
/// Output streams from subprograms must be connected to the input of a program that accepts that message type
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct TargetInputMessageType(pub String);

///
/// The name of the message type that is being connected to a target
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SourceStreamMessageType(pub String);

///
/// Errors that can occur when trying to connect two subprograms in a scene
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub enum ConnectionError {
    // Something cancelled the connection
    Cancelled,

    /// The subprogram a context belongs to is no longer running
    SubProgramNotRunning,

    /// The input type of the target of a connection does not match the source
    WrongInputType(SourceStreamMessageType, TargetInputMessageType),

    /// The target subprogram of a connection is not in the scene (has not been started, or has finished)
    TargetNotInScene,

    /// The target input stream is not available
    TargetNotAvailable,

    /// The input to a filter does not match what was expected
    FilterInputDoesNotMatch,

    /// The output to a filter does not match what was expected
    FilterOutputDoesNotMatch,

    /// The filter handle was not found
    FilterHandleNotFound,

    /// A filter to map from one stream to another was expected to be defined but could not be found
    FilterMappingMissing,

    /// The input for the filter to a filter source must match the stream ID being connected
    FilterSourceInputMustMatchStream,

    /// The input for the filter to a filter target must match the stream ID being connected
    FilterTargetInputMustMatchStream,

    /// A stream target had an unexpected value
    UnexpectedConnectionType,

    /// The `OUTSIDE_SCENE_PROGRAM` subprogram is not running and a sink for sending messages into the scene was requested
    NoOutsideSceneSubProgram,

    /// An attempt was made to 'steal' the current thread to expedite a message, which could not be completed (for example, because the subprogram was already running on the current thread)
    CannotStealThread,

    /// The connection is denied due to a permissions error
    TargetPermissionRefused,

    /// The target refused the connection
    TargetConnectionRefused,

    /// An operation could not be completed because of an I/O problem
    IoError(String),
}

///
/// Error that occurs while sending to a stream
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub enum SceneSendError<TMessage> {
    /// The target program ended while waiting for it to become ready (or after sending the message but before it could be flushed)
    TargetProgramEndedBeforeReady,

    /// The target for the stream stopped before the message could be sent
    TargetProgramEnded(TMessage),

    /// The stream is disconnected, so messages cannot currently be sent to it
    StreamDisconnected(TMessage),

    /// The target program supports thread stealing, but it is already running on the current thread's callstack and can't re-enter
    CannotReEnterTargetProgram,
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
            SceneSendError::TargetProgramEndedBeforeReady   => None,
            SceneSendError::TargetProgramEnded(msg)         => Some(msg),
            SceneSendError::StreamDisconnected(msg)         => Some(msg),
            SceneSendError::CannotReEnterTargetProgram      => None,
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
            SceneSendError::TargetProgramEndedBeforeReady   => None,
            SceneSendError::TargetProgramEnded(msg)         => Some(msg),
            SceneSendError::StreamDisconnected(msg)         => Some(msg),
            SceneSendError::CannotReEnterTargetProgram      => None,
        }
    }
}

impl<TMessage> From<SceneSendError<TMessage>> for ConnectionError {
    fn from(err: SceneSendError<TMessage>) -> ConnectionError {
        match err {
            SceneSendError::TargetProgramEndedBeforeReady   => ConnectionError::TargetNotInScene,
            SceneSendError::TargetProgramEnded(_)           => ConnectionError::TargetNotInScene,
            SceneSendError::StreamDisconnected(_)           => ConnectionError::TargetNotAvailable,
            SceneSendError::CannotReEnterTargetProgram      => ConnectionError::CannotStealThread,
        }
    }
}

#[cfg(feature="tokio")]
mod tokio_errors {
    use super::*;
    use tokio::io::{Error, ErrorKind};

    impl From<Error> for ConnectionError {
        fn from(err: Error) -> ConnectionError {
            match err.kind() {
                ErrorKind::NotFound             => ConnectionError::TargetNotAvailable,
                ErrorKind::PermissionDenied     => ConnectionError::TargetPermissionRefused,
                ErrorKind::ConnectionRefused    => ConnectionError::TargetConnectionRefused,
                ErrorKind::ConnectionReset      |
                ErrorKind::BrokenPipe           |
                ErrorKind::ConnectionAborted    => ConnectionError::Cancelled,
                ErrorKind::NotConnected         => ConnectionError::IoError(format!("{}", err)),
                ErrorKind::AddrInUse            |
                ErrorKind::AddrNotAvailable     |
                ErrorKind::AlreadyExists        |
                ErrorKind::WouldBlock           |
                ErrorKind::InvalidInput         |
                ErrorKind::InvalidData          |
                ErrorKind::TimedOut             |
                ErrorKind::WriteZero            |
                ErrorKind::Interrupted          |
                ErrorKind::Unsupported          |
                ErrorKind::UnexpectedEof        |
                ErrorKind::OutOfMemory          |
                ErrorKind::Other                |
                _                               => ConnectionError::IoError(err.to_string()),

            }
        }
    }
}

#[cfg(feature="tokio")]
pub use tokio_errors::*;

///
/// The name of the message type that is accepted by a subprogram
///
/// Output streams from subprograms must be connected to the input of a program that accepts that message type
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TargetInputMessageType(pub String);

///
/// The name of the message type that is being connected to a target
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SourceStreamMessageType(pub String);

///
/// Errors that can occur when trying to connect two subprograms in a scene
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ConnectionError {
    // Something cancelled the connection
    Cancelled,

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
pub enum SceneSendError {
    /// The target for the stream stopped before the message could be sent
    TargetProgramEnded,

    /// The stream is disconnected, so messages cannot currently be sent to it
    StreamDisconnected,

    /// The target program supports thread stealing, but it is already running on the current thread's callstack and can't re-enter
    CannotReEnterTargetProgram,
}

impl From<SceneSendError> for ConnectionError {
    fn from(err: SceneSendError) -> ConnectionError {
        match err {
            SceneSendError::TargetProgramEnded          => ConnectionError::TargetNotInScene,
            SceneSendError::StreamDisconnected          => ConnectionError::TargetNotAvailable,
            SceneSendError::CannotReEnterTargetProgram  => ConnectionError::CannotStealThread,
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

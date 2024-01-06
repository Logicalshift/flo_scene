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

    /// A stream target had an unexpected value
    UnexpectedConnectionType,

    /// The `OUTSIDE_SCENE_PROGRAM` subprogram is not running and a sink for sending messages into the scene was requested
    NoOutsideSceneSubProgram,
}

///
/// Error that occurs while sending to a stream
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum SceneSendError {
    /// The target for the stream stopped before the message could be sent
    TargetProgramEnded
}

impl From<SceneSendError> for ConnectionError {
    fn from(err: SceneSendError) -> ConnectionError {
        match err {
            SceneSendError::TargetProgramEnded => ConnectionError::TargetNotInScene,
        }
    }
}

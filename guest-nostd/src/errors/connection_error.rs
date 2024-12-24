use serde::*;

use alloc::string::*;

///
/// The name of the message type that is accepted by a subprogram
///
/// Output streams from subprograms must be connected to the input of a program that accepts that message type
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
pub struct TargetInputMessageType(pub String);

///
/// The name of the message type that is being connected to a target
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
pub struct SourceStreamMessageType(pub String);

///
/// Errors that can occur when trying to connect two subprograms in a scene
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
pub enum ConnectionError {
    // Something cancelled the connection
    Cancelled,

    /// The subprogram a context belongs to is no longer running
    SubProgramNotRunning,

    /// The input type of the target of a connection does not match the source
    WrongInputType(SourceStreamMessageType, TargetInputMessageType),

    /// The requested stream is not available
    StreamNotKnown,

    /// The target subprogram of a connection is not in the scene (has not been started, or has finished)
    TargetNotInScene,

    /// The target input stream is not available
    TargetNotAvailable,

    /// The target cannot accept a message because it's not ready
    TargetNotReady,

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

    /// The filter supplied is not supported
    FilterNotSupported,

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

    /// The target doesn't support serializing this message type
    TargetCannotSerialize,

    /// The target doesn't support receiving serialized messages
    TargetCannotDeserialize,

    /// An operation could not be completed because of an I/O problem
    IoError(String),
}

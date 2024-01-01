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
}

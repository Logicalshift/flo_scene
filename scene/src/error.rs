///
/// Errors that can occur when trying to connect two subprograms in a scene
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ConnectionError {
    /// The input type of the target of a connection does not match the source
    WrongInputType,

    /// The target subprogram of a connection is not in the scene (has not been started, or has finished)
    TargetNotInScene,

    /// The target input stream is not available
    TargetNotAvailable,
}

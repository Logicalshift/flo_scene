///
/// The possible outcomes of a successful connection request
///
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ConnectResult {
    /// The connection was made and the target was ready to receive inputs
    Ready,

    /// The target for the connection is not ready, so messages sent to the connection will block until the target subprogram starts
    TargetNotReady,

    /// The target exists but does not accept connections of this type, so messages sent to the connection will block until a filter is added that supports this input type
    InputTypeNotReady,
}

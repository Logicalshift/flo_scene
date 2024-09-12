///
/// Identifies a stream on the host side
///
/// We use the serialization name for the stream to identify which stream it is. Guest programs may communicate with
/// each other using a stream type that's not known to the host (connections are passed without any processing if the
/// stream ID matches on the other side of the connection)
///
#[derive(Clone, PartialEq, Debug, Eq, PartialOrd, Ord, Hash)]
pub struct HostStreamId(pub String);

impl HostStreamId {
    ///
    /// Creates a host stream ID using a type name
    ///
    #[inline]
    pub fn with_name(name: impl Into<String>) -> Self {
        HostStreamId(name.into())
    }
}
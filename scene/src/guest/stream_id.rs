///
/// Identifies a stream on the host side (we just use a string name corresponding to the serialization name for the stream)
///
#[derive(Clone, PartialEq, Debug)]
pub struct HostStreamId(pub String);

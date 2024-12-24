use serde::*;

///
/// Identifies a serialized resource
///
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum SerializationId {
    /// Identifies a stream whose source is on this side of the connection
    MyStream(usize),

    /// Identifies a stream whose source is on the target side of the connection
    ///
    /// Streams are 'inverted' after they are sent across a connection, so when we're serializing a value to send to a guest (or a host), we always
    /// send a 'TheirStream' as they will be accessed on the other side of the connection.
    TheirStream(usize),
}

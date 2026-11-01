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

impl SerializationId {
    ///
    /// Returns the 'opposite' of the current stream (converts 'MyStream' to 'TheirStream' and vice-vera)
    ///
    pub fn invert(&self) -> Self {
        match self {
            Self::MyStream(id)      => Self::TheirStream(*id),
            Self::TheirStream(id)   => Self::MyStream(*id),
        }
    }

    ///
    /// Converts a serialization ID to one that's owned on 'this side' of the connection
    ///
    pub fn to_mine(&self) -> Self {
        match self {
            Self::MyStream(id) | Self::TheirStream(id) => Self::MyStream(*id)
        }
    }

    ///
    /// Converts a serialization ID to one that's owned on the 'other side' of the connection
    ///
    pub fn to_theirs(&self) -> Self {
        match self {
            Self::MyStream(id) | Self::TheirStream(id) => Self::TheirStream(*id)
        }
    }
}

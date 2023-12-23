use std::any::{TypeId};

///
/// Identifies a stream produced by a subprogram 
///
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StreamId {
    /// A stream identified by its message type
    MessageType(TypeId)
}

impl StreamId {
    ///
    /// ID of a stream that generates a particular type of data
    ///
    pub fn with_message_type<TMessageType>() -> Self 
    where
        TMessageType: 'static,
    {
        StreamId::MessageType(TypeId::of::<TMessageType>())
    }
}

impl From<TypeId> for StreamId {
    #[inline]
    fn from(type_id: TypeId) -> StreamId {
        StreamId::MessageType(type_id)
    }
}
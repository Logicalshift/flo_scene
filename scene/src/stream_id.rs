use crate::stream_target::*;

use std::any::{TypeId};

///
/// Identifies a stream produced by a subprogram 
///
#[derive(Clone, PartialEq, Hash, Debug)]
pub enum StreamId {
    /// A stream identified by its message type
    MessageType(TypeId),

    /// A stream sending data to a specific target
    Target(TypeId, StreamTarget),
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

    ///
    /// ID of a stream that is assigned to a particular target
    ///
    pub fn for_target<TMessageType>(target: impl Into<StreamTarget>) -> Self
    where
        TMessageType: 'static,
    {
        StreamId::Target(TypeId::of::<TMessageType>(), target.into())
    }
}

impl From<TypeId> for StreamId {
    #[inline]
    fn from(type_id: TypeId) -> StreamId {
        StreamId::MessageType(type_id)
    }
}
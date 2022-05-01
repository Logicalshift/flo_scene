use crate::entity_channel::*;
use crate::mapped_entity_channel::*;

///
/// Extensions added to all entity channels
///
pub trait EntityChannelExt : Sized + EntityChannel {
    ///
    /// Applies a mapping function to an entity channel, changing its type and optionally processing the message and response
    ///
    fn map<TMessageFn, TResponseFn, TNewMessage, TNewResponse>(self, message_map: TMessageFn, response_map: TResponseFn) -> MappedEntityChannel<Self, TMessageFn, TResponseFn, TNewMessage>
    where
        TNewMessage:    Send,
        TNewResponse:   Send,
        TMessageFn:     Send + Fn(TNewMessage) -> Self::Message,
        TResponseFn:    Send + Fn(Self::Response) -> TNewResponse;

    ///
    /// Puts this channel in a box
    ///
    fn boxed<'a>(self) -> BoxedEntityChannel<'a, Self::Message, Self::Response>
    where
        Self: 'a;
}

impl<T> EntityChannelExt for T
where
    T : EntityChannel
{
    fn map<TMessageFn, TResponseFn, TNewMessage, TNewResponse>(self, message_map: TMessageFn, response_map: TResponseFn) -> MappedEntityChannel<Self, TMessageFn, TResponseFn, TNewMessage>
    where
        TNewMessage:    Send,
        TNewResponse:   Send,
        TMessageFn:     Send + Fn(TNewMessage) -> Self::Message,
        TResponseFn:    Send + Fn(Self::Response) -> TNewResponse {
        MappedEntityChannel::new(self, message_map, response_map)
    }

    fn boxed<'a>(self) -> BoxedEntityChannel<'a, Self::Message, Self::Response> 
    where
        Self: 'a
    {
        Box::new(self)
    }
}

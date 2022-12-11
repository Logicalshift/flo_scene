use crate::entity_channel::*;
use crate::mapped_entity_channel::*;
use crate::convert_entity_channel::*;

///
/// Extensions added to all entity channels
///
pub trait EntityChannelExt : Sized + EntityChannel {
    ///
    /// Applies a mapping function to an entity channel, changing its type and optionally processing the message
    ///
    fn map<TMessageFn, TNewMessage>(self, message_map: TMessageFn) -> MappedEntityChannel<Self, TMessageFn, TNewMessage>
    where
        TNewMessage:    Send,
        TMessageFn:     Send + Fn(TNewMessage) -> Self::Message;

    ///
    /// Converts this entity channel to another of a compatible type
    ///
    fn convert<TNewMessage>(self) -> ConvertEntityChannel<Self, TNewMessage>
    where
        Self::Message:  From<TNewMessage>,
        TNewMessage:    Send;

    ///
    /// Converts this entity channel to another of a compatible type, by changing the message type only
    ///
    fn convert_message<TNewMessage>(self) -> ConvertEntityChannel<Self, TNewMessage>
    where
        Self::Message:  From<TNewMessage>,
        TNewMessage:    Send;

    ///
    /// Puts this channel in a box
    ///
    fn boxed<'a>(self) -> BoxedEntityChannel<'a, Self::Message>
    where
        Self: 'a;
}

impl<T> EntityChannelExt for T
where
    T : EntityChannel
{
    fn map<TMessageFn, TNewMessage>(self, message_map: TMessageFn) -> MappedEntityChannel<Self, TMessageFn, TNewMessage>
    where
        TNewMessage:    Send,
        TMessageFn:     Send + Fn(TNewMessage) -> Self::Message,
    {
        MappedEntityChannel::new(self, message_map)
    }

    fn convert<TNewMessage>(self) -> ConvertEntityChannel<Self, TNewMessage>
    where
        Self::Message:  From<TNewMessage>,
        TNewMessage:    Send,
    {
        ConvertEntityChannel::new(self)
    }

    fn convert_message<TNewMessage>(self) -> ConvertEntityChannel<Self, TNewMessage>
    where
        Self::Message:  From<TNewMessage>,
        TNewMessage:    Send,
    {
        ConvertEntityChannel::new(self)
    }

    fn boxed<'a>(self) -> BoxedEntityChannel<'a, Self::Message> 
    where
        Self: 'a
    {
        Box::new(self)
    }
}

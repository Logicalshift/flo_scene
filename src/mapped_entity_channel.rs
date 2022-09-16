use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::marker::{PhantomData};

///
/// Maps an entity channel to another type
///
pub struct MappedEntityChannel<TSourceChannel, TMapMessageFn, TNewMessage> {
    source_channel:         TSourceChannel,
    map_message:            TMapMessageFn,
    new_message_phantom:    PhantomData<TNewMessage>,
}

impl<TSourceChannel, TMapMessageFn, TNewMessage> MappedEntityChannel<TSourceChannel, TMapMessageFn, TNewMessage>
where
    TSourceChannel:             EntityChannel,
    TSourceChannel::Message:    Send,
    TNewMessage:                Send,
    TMapMessageFn:              Send + Fn(TNewMessage) -> TSourceChannel::Message,
{
    ///
    /// Creates a new mapped entity channel
    ///
    pub fn new(source_channel: TSourceChannel, map_message: TMapMessageFn) -> MappedEntityChannel<TSourceChannel, TMapMessageFn, TNewMessage> {
        MappedEntityChannel {
            source_channel,
            map_message,
            new_message_phantom: PhantomData,
        }
    }
}

impl<TSourceChannel, TMapMessageFn, TNewMessage> EntityChannel for MappedEntityChannel<TSourceChannel, TMapMessageFn, TNewMessage> 
where
    TSourceChannel:             EntityChannel,
    TSourceChannel::Message:    Send,
    TNewMessage:                Send,
    TMapMessageFn:              Send + Fn(TNewMessage) -> TSourceChannel::Message,
{
    type Message    = TNewMessage;

    fn entity_id(&self) -> EntityId {
        self.source_channel.entity_id()
    }

    fn is_closed(&self) -> bool {
        self.source_channel.is_closed()
    }

    fn send(&mut self, message: TNewMessage) -> BoxFuture<'static, Result<(), EntityChannelError>> {
        let message = (&self.map_message)(message);
        let future  = self.source_channel.send(message);

        async move {
            future.await?;

            Ok(())
        }.boxed()
    }
}

use crate::error::*;
use crate::entity_channel::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::marker::{PhantomData};

///
/// Maps an entity channel to another type
///
pub struct MappedEntityChannel<TSourceChannel, TMapMessageFn, TMapResponseFn, TNewMessage> {
    source_channel:         TSourceChannel,
    map_message:            TMapMessageFn,
    map_response:           TMapResponseFn,
    new_message_phantom:    PhantomData<TNewMessage>,
}

impl<TSourceChannel, TMapMessageFn, TMapResponseFn, TNewMessage, TNewResponse> MappedEntityChannel<TSourceChannel, TMapMessageFn, TMapResponseFn, TNewMessage>
where
    TSourceChannel:             EntityChannel,
    TSourceChannel::Message:    Send,
    TNewMessage:                Send,
    TNewResponse:               Send,
    TMapMessageFn:              Send + Fn(TNewMessage) -> TSourceChannel::Message,
    TMapResponseFn:             Send + Fn(TSourceChannel::Response) -> TNewResponse,
{
    ///
    /// Creates a new mapped entity channel
    ///
    pub fn new(source_channel: TSourceChannel, map_message: TMapMessageFn, map_response: TMapResponseFn) -> MappedEntityChannel<TSourceChannel, TMapMessageFn, TMapResponseFn, TNewMessage> {
        MappedEntityChannel {
            source_channel,
            map_message,
            map_response,
            new_message_phantom: PhantomData,
        }
    }
}

impl<TSourceChannel, TMapMessageFn, TMapResponseFn, TNewMessage, TNewResponse> EntityChannel for MappedEntityChannel<TSourceChannel, TMapMessageFn, TMapResponseFn, TNewMessage> 
where
    TSourceChannel:             EntityChannel,
    TSourceChannel::Message:    Send,
    TNewMessage:                Send,
    TNewResponse:               Send,
    TMapMessageFn:              Send + Fn(TNewMessage) -> TSourceChannel::Message,
    TMapResponseFn:             Send + Fn(TSourceChannel::Response) -> TNewResponse,
{
    type Message    = TNewMessage;
    type Response   = TNewResponse;

    fn send<'a>(&'a mut self, message: TNewMessage) -> BoxFuture<'a, Result<Self::Response, EntityChannelError>> {
        async move {
            let message     = (&self.map_message)(message);
            let response    = self.source_channel.send(message).await?;
            let response    = (&self.map_response)(response);

            Ok(response)
        }.boxed()
    }

    fn send_without_waiting<'a>(&'a mut self, message: TNewMessage) -> BoxFuture<'a, Result<(), EntityChannelError>> {
        async move {
            let message = (&self.map_message)(message);
            self.source_channel.send(message).await?;

            Ok(())
        }.boxed()
    }
}

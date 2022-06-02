use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::marker::{PhantomData};

///
/// Converts an entity channel from one type to another
///
pub struct ConvertEntityChannel<TSourceChannel, TNewMessage, TNewResponse> {
    source_channel: TSourceChannel,
    new_message:    PhantomData<TNewMessage>,
    new_response:   PhantomData<TNewResponse>,
}

impl<TSourceChannel, TNewMessage, TNewResponse> ConvertEntityChannel<TSourceChannel, TNewMessage, TNewResponse>
where
    TSourceChannel:             EntityChannel,
    TSourceChannel::Message:    From<TNewMessage>,
    TSourceChannel::Response:   Into<TNewResponse>,
    TNewMessage:                Send,
    TNewResponse:               Send,
{
    ///
    /// Creates a new convertion entity channel
    ///
    pub fn new(source_channel: TSourceChannel) -> ConvertEntityChannel<TSourceChannel, TNewMessage, TNewResponse> {
        ConvertEntityChannel {
            source_channel: source_channel,
            new_message:    PhantomData,
            new_response:   PhantomData,
        }
    }
}

impl<TSourceChannel, TNewMessage, TNewResponse> EntityChannel for ConvertEntityChannel<TSourceChannel, TNewMessage, TNewResponse>
where
    TSourceChannel:             EntityChannel,
    TSourceChannel::Message:    From<TNewMessage>,
    TSourceChannel::Response:   Into<TNewResponse>,
    TNewMessage:                Send,
    TNewResponse:               Send,
{
    type Message    = TNewMessage;
    type Response   = TNewResponse;

    fn entity_id(&self) -> EntityId {
        self.source_channel.entity_id()
    }

    fn send<'a>(&'a mut self, message: TNewMessage) -> BoxFuture<'a, Result<Self::Response, EntityChannelError>> {
        async move {
            let message     = TSourceChannel::Message::from(message);
            let response    = self.source_channel.send(message).await?;
            let response    = response.into();

            Ok(response)
        }.boxed()
    }

    fn send_without_waiting(&mut self, message: Self::Message) -> BoxFuture<'static, Result<(), EntityChannelError>> {
        let message = TSourceChannel::Message::from(message);
        let future  = self.source_channel.send_without_waiting(message);

        async move {
            future.await?;

            Ok(())
        }.boxed()
    }
}

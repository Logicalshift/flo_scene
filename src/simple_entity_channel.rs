use crate::error::*;
use crate::message::*;
use crate::entity_id::*;
use crate::entity_channel::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::channel::mpsc;

use std::mem;

///
/// A simple entity channel just relays messages directly to a target channel
///
pub struct SimpleEntityChannel<TMessage, TResponse> {
    /// The channel for sending messages
    channel: mpsc::Sender<Message<TMessage, TResponse>>,

    /// The entity ID that owns this channel
    entity_id: EntityId,
}

impl<TMessage, TResponse> SimpleEntityChannel<TMessage, TResponse> {
    ///
    /// Creates a new entity channel
    ///
    pub fn new(entity_id: EntityId, buf_size: usize) -> (SimpleEntityChannel<TMessage, TResponse>, mpsc::Receiver<Message<TMessage, TResponse>>) {
        let (sender, receiver) = mpsc::channel(buf_size);

        let channel = SimpleEntityChannel {
            channel:    sender,
            entity_id:  entity_id,
        };

        (channel, receiver)
    }

    ///
    /// Closes this channel
    ///
    pub fn close(&mut self) {
        self.channel.close_channel();
    }
}

impl<TMessage, TResponse> EntityChannel for SimpleEntityChannel<TMessage, TResponse> 
where
    TMessage:   'static + Send,
    TResponse:  'static + Send,
{
    type Message    = TMessage;
    type Response   = TResponse;

    fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    fn send<'a>(&'a mut self, message: TMessage) -> BoxFuture<'a, Result<TResponse, EntityChannelError>> {
        async move {
            // Wrap the request into a message
            let (message, receiver) = Message::new(message);

            // Send the message to the channel
            self.channel.send(message).await?;

            // Wait for the message to be processed
            receiver.await.map_err(|_cancelled| EntityChannelError::NoResponse)
        }.boxed()
    }

    fn send_without_waiting<'a>(&'a mut self, message: Self::Message) -> BoxFuture<'a, Result<(), EntityChannelError>> {
        async move {
            // Wrap the request into a message
            let (message, receiver) = Message::new(message);

            // Don't care about the response
            mem::drop(receiver);

            // Send the message to the channel
            if let Err(err) = self.channel.try_send(message) {
                self.channel.send(err.into_inner()).await?;
            }

            Ok(())
        }.boxed()
    }
}

impl<TMessage, TResponse> Clone for SimpleEntityChannel<TMessage, TResponse> {
    fn clone(&self) -> Self {
        SimpleEntityChannel {
            channel:    self.channel.clone(),
            entity_id:  self.entity_id,
        }
    }
}

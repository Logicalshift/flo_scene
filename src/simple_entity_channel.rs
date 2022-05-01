use crate::error::*;
use crate::message::*;
use crate::entity_channel::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::channel::mpsc;

///
/// A simple entity channel just relays messages directly to a target channel
///
pub struct SimpleEntityChannel<TMessage, TResponse> {
    /// The channel for sending messages
    channel: mpsc::Sender<Message<TMessage, TResponse>>,
}

impl<TMessage, TResponse> SimpleEntityChannel<TMessage, TResponse> {
    ///
    /// Creates a new entity channel
    ///
    pub fn new(buf_size: usize) -> (SimpleEntityChannel<TMessage, TResponse>, mpsc::Receiver<Message<TMessage, TResponse>>) {
        let (sender, receiver) = mpsc::channel(buf_size);

        let channel = SimpleEntityChannel {
            channel: sender
        };

        (channel, receiver)
    }
}

impl<TMessage, TResponse> EntityChannel for SimpleEntityChannel<TMessage, TResponse> 
where
    TMessage:   'static + Send,
    TResponse:  'static + Send,
{
    type Message    = TMessage;
    type Response   = TResponse;

    ///
    /// Sends a message to the channel and waits for a response
    ///
    fn send<'a>(&'a mut self, message: TMessage) -> BoxFuture<'a, Result<TResponse, EntityChannelError>> {
        async move {
            // Wrap the request into a message
            let (message, receiver) = Message::new(message);

            // Send the message to the channel
            self.channel.send(message).await?;

            // Wait for the message to be processed
            Ok(receiver.await?)
        }.boxed()
    }
}

impl<TMessage, TResponse> Clone for SimpleEntityChannel<TMessage, TResponse> {
    fn clone(&self) -> Self {
        SimpleEntityChannel {
            channel: self.channel.clone()
        }
    }
}

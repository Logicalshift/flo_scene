use crate::error::*;
use crate::message::*;

use futures::prelude::*;
use futures::channel::mpsc;

///
/// An entity channel is used to send messages to an entity within a scene
///
pub struct EntityChannel<TMessage, TResponse> {
    /// The channel for sending messages
    channel: mpsc::Sender<Message<TMessage, TResponse>>,
}

impl<TMessage, TResponse> EntityChannel<TMessage, TResponse> {
    ///
    /// Sends a message to the channel and waits for a response
    ///
    pub async fn send(&mut self, message: TMessage) -> Result<TResponse, EntityChannelError> {
        // Wrap the request into a message
        let (message, receiver) = Message::new(message);

        // Send the message to the channel
        self.channel.send(message).await?;

        // Wait for the message to be processed
        Ok(receiver.await?)
    }
}

impl<TMessage, TResponse> Clone for EntityChannel<TMessage, TResponse> {
    fn clone(&self) -> Self {
        EntityChannel {
            channel: self.channel.clone()
        }
    }
}

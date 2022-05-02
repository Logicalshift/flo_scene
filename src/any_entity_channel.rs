use crate::error::*;
use crate::entity_channel::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::any::{Any, type_name};

///
/// Accepts a dynamically typed message and produces a dynamically typed response
///
/// This takes a type of `Option<Message>` and returns a response of `Option<Response>`, boxed up as `Box<dyn Send + Any>`.
/// The option type is used so that the underlying message and response can be extracted. This is generally used as an
/// intermediate stage for converting a channel between types.
///
pub struct AnyEntityChannel {
    /// The dynamic send function for this channel
    send: Box<dyn Send + Fn(Box<dyn Send + Any>) -> BoxFuture<'static, Result<Box<dyn Send + Any>, EntityChannelError>>>,

    /// The dynamic send function for this channel
    send_without_waiting: Box<dyn Send + Fn(Box<dyn Send + Any>) -> BoxFuture<'static, Result<(), EntityChannelError>>>,
}

impl AnyEntityChannel {
    ///
    /// Converts a channel to an 'any' channel
    ///
    pub fn from_channel<TChannel: EntityChannel>(channel: TChannel) -> AnyEntityChannel 
    where
        TChannel:           'static + Clone,
        TChannel::Message:  'static,
        TChannel::Response: 'static + Sized,
    {
        let send_channel = channel.clone();
        let send = Box::new(move |boxed_message: Box<dyn Send + Any>| {
            let mut channel = send_channel.clone();

            async move {
                // Extract the message components
                let mut message         = boxed_message;

                // Unbox the request. We use `Option<TChannel::Message>` so we can take the message out of the box
                if let Some(message) = message.downcast_mut::<Option<TChannel::Message>>() {
                    if let Some(message) = message.take() {
                        // Send the message
                        let response = channel.send(message).await?;

                        // Box up the response. We use `Option<TChannel::Response>` so the receiver can take the response out of the box.
                        let response: Box<dyn Send + Any> = Box::new(Some(response));

                        Ok(response)
                    } else {
                        // The message was missing
                        Err(EntityChannelError::MissingMessage)
                    }
                } else {
                    // Did not downcast
                    Err(EntityChannelError::WrongMessageType(format!("{}", type_name::<TChannel::Message>())))
                }
            }.boxed()
        });

        let send_without_waiting = Box::new(move |boxed_message: Box<dyn Send + Any>| {
            let mut channel = channel.clone();

            async move {
                // Extract the message components
                let mut message         = boxed_message;

                // Unbox the request. We use `Option<TChannel::Message>` so we can take the message out of the box
                if let Some(message) = message.downcast_mut::<Option<TChannel::Message>>() {
                    if let Some(message) = message.take() {
                        // Send the message
                        channel.send_without_waiting(message).await?;

                        Ok(())
                    } else {
                        // The message was missing
                        Err(EntityChannelError::MissingMessage)
                    }
                } else {
                    // Did not downcast
                    Err(EntityChannelError::WrongMessageType(format!("{}", type_name::<TChannel::Message>())))
                }
            }.boxed()
        });

        AnyEntityChannel {
            send,
            send_without_waiting,
        }
    }
}

impl EntityChannel for AnyEntityChannel {
    type Message    = Box<dyn Send + Any>;
    type Response   = Box<dyn Send + Any>;

    #[inline]
    fn send<'a>(&'a mut self, message: Box<dyn Send + Any>) -> BoxFuture<'a, Result<Box<dyn Send + Any>, EntityChannelError>> {
        (self.send)(message)
    }

    #[inline]
    fn send_without_waiting<'a>(&'a mut self, message: Box<dyn Send + Any>) -> BoxFuture<'a, Result<(), EntityChannelError>> {
        (self.send_without_waiting)(message)
    }
}

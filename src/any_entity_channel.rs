use crate::error::*;
use crate::entity_id::*;
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
    /// The ID of the entity for this channel
    entity_id: EntityId,

    /// Returns whether or not the original channel is closed
    is_closed: Box<dyn Send + Fn() -> bool>,

    /// The dynamic send function for this channel
    send: Box<dyn Send + Fn(Box<dyn Send + Any>) -> BoxFuture<'static, Result<Box<dyn Send + Any>, EntityChannelError>>>,

    /// The dynamic send function for this channel
    send_without_waiting: Box<dyn Send + Fn(Box<dyn Send + Any>) -> BoxFuture<'static, Result<(), EntityChannelError>>>,
}

impl AnyEntityChannel {
    ///
    /// Converts a channel to an 'any' channel
    ///
    pub fn from_channel<TChannel>(channel: TChannel) -> AnyEntityChannel 
    where
        TChannel:           'static + EntityChannel + Clone,
        TChannel::Message:  'static,
        TChannel::Response: 'static + Sized,
    {
        let entity_id       = channel.entity_id();

        let closed_channel  = channel.clone();
        let is_closed       = Box::new(move || {
            closed_channel.is_closed()
        });

        let send_channel    = channel.clone();
        let send            = Box::new(move |boxed_message: Box<dyn Send + Any>| {
            let mut channel = send_channel.clone();

            // Extract the message components
            let mut message         = boxed_message;

            // Unbox the request. We use `Option<TChannel::Message>` so we can take the message out of the box
            let send_future = if let Some(message) = message.downcast_mut::<Option<TChannel::Message>>() {
                if let Some(message) = message.take() {
                    // Send the message
                    // TODO: we need to call channel.send() immediately, but this causes lifetime issues with 'channel', which we can't move into the future
                    Ok(async move { channel.send(message).await })
                } else {
                    // The message was missing
                    Err(EntityChannelError::MissingMessage)
                }
            } else {
                // Did not downcast
                Err(EntityChannelError::WrongMessageType(format!("{}", type_name::<TChannel::Message>())))
            };

            async move {
                // Wait for the future we just created
                let response = send_future?.await?;

                // Box up the response. We use `Option<TChannel::Response>` so the receiver can take the response out of the box.
                let response: Box<dyn Send + Any> = Box::new(Some(response));

                Ok(response)
            }.boxed()
        });

        let send_without_waiting = Box::new(move |boxed_message: Box<dyn Send + Any>| {
            let mut channel = channel.clone();

            // Extract the message components
            let mut message         = boxed_message;

            // Unbox the request. We use `Option<TChannel::Message>` so we can take the message out of the box
            let send_future = if let Some(message) = message.downcast_mut::<Option<TChannel::Message>>() {
                if let Some(message) = message.take() {
                    // Create the future to send the message
                    Ok(channel.send_without_waiting(message))
                } else {
                    // The message was missing
                    Err(EntityChannelError::MissingMessage)
                }
            } else {
                // Did not downcast
                Err(EntityChannelError::WrongMessageType(format!("{}", type_name::<TChannel::Message>())))
            };

            async move {
                send_future?.await?;

                Ok(())
            }.boxed()
        });

        AnyEntityChannel {
            entity_id,
            is_closed,
            send,
            send_without_waiting,
        }
    }
}

impl EntityChannel for AnyEntityChannel {
    type Message    = Box<dyn Send + Any>;
    type Response   = Box<dyn Send + Any>;

    #[inline]
    fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    #[inline]
    fn is_closed(&self) -> bool {
        (self.is_closed)()
    }

    #[inline]
    fn send<'a>(&'a mut self, message: Box<dyn Send + Any>) -> BoxFuture<'a, Result<Box<dyn Send + Any>, EntityChannelError>> {
        (self.send)(message)
    }

    #[inline]
    fn send_without_waiting(&mut self, message: Box<dyn Send + Any>) -> BoxFuture<'static, Result<(), EntityChannelError>> {
        (self.send_without_waiting)(message)
    }
}

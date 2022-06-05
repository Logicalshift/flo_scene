use crate::error::*;
use crate::entity_id::*;

use futures::future::{BoxFuture};

use std::fmt;
use std::any;

///
/// EntityChannel is a trait implemented by structures that can send messages to entities within a scene
///
pub trait EntityChannel : Send {
    /// The type of message that can be sent to this channel
    type Message: Send;

    /// The type of response that this channel will generate
    type Response: Send;

    ///
    /// Returns the ID of the entity that will receive messages from this channel
    ///
    fn entity_id(&self) -> EntityId;

    ///
    /// True if this channel has been closed
    ///
    fn is_closed(&self) -> bool;

    ///
    /// Sends a message to the channel and waits for a response
    ///
    fn send<'a>(&'a mut self, message: Self::Message) -> BoxFuture<'a, Result<Self::Response, EntityChannelError>>;

    ///
    /// Sends a message to a channel where we don't want to wait for a response
    ///
    /// This is most useful for cases where the response is '()' - indeed, the version in `SceneContext` only supports
    /// this version. Not waiting for a response is often a faster way to dispatch messages, and also prevents deadlocks
    /// in the event that the message triggers a callback to the original entity. This also doesn't generate an error
    /// in the event the channel drops the message without responding to it.
    ///
    fn send_without_waiting(&mut self, message: Self::Message) -> BoxFuture<'static, Result<(), EntityChannelError>>;
}

///
/// A boxed entity channel is used to hide the real type of an entity channel
///
pub type BoxedEntityChannel<'a, TMessage, TResponse> = Box<dyn 'a + EntityChannel<Message=TMessage, Response=TResponse>>;

impl<'a, TMessage, TResponse> EntityChannel for BoxedEntityChannel<'a, TMessage, TResponse> 
where
    TMessage:  Send,
    TResponse: Send,
{
    type Message    = TMessage;
    type Response   = TResponse;

    #[inline]
    fn entity_id(&self) -> EntityId {
        (**self).entity_id()
    }

    #[inline]
    fn is_closed(&self) -> bool {
        (**self).is_closed()
    }

    #[inline]
    fn send<'b>(&'b mut self, message: Self::Message) -> BoxFuture<'b, Result<Self::Response, EntityChannelError>> {
        (**self).send(message)
    }

    #[inline]
    fn send_without_waiting(&mut self, message: Self::Message) -> BoxFuture<'static, Result<(), EntityChannelError>> {
        (**self).send_without_waiting(message)
    }
}

impl<'a, TMessage, TResponse> fmt::Debug for BoxedEntityChannel<'a, TMessage, TResponse> 
where
    TMessage:  Send,
    TResponse: Send,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.write_fmt(format_args!("BoxedEntityChannel::<{}, {}>( -> {:?})", any::type_name::<TMessage>(), any::type_name::<TResponse>(), self.entity_id()))
    }
}

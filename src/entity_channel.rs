use crate::error::*;

use futures::future::{BoxFuture};

///
/// EntityChannel is a trait implemented by structures that can send messages to entities within a scene
///
pub trait EntityChannel : Send {
    /// The type of message that can be sent to this channel
    type Message: Send;

    /// The type of response that this channel will generate
    type Response: Send;

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
    fn send_without_waiting<'a>(&'a mut self, message: Self::Message) -> BoxFuture<'a, Result<(), EntityChannelError>>;
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
    fn send<'b>(&'b mut self, message: Self::Message) -> BoxFuture<'b, Result<Self::Response, EntityChannelError>> {
        (**self).send(message)
    }

    #[inline]
    fn send_without_waiting<'b>(&'b mut self, message: Self::Message) -> BoxFuture<'b, Result<(), EntityChannelError>> {
        (**self).send_without_waiting(message)
    }
}

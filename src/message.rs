use futures::channel::oneshot;

use std::fmt;
use std::ops::{Deref, DerefMut};

///
/// Messages are the main means that entities use to communicate with one another
///
pub struct Message<TPayload, TResponse> {
    /// The data for this message
    message: TPayload,

    /// The response that should be sent for this message
    response: oneshot::Sender<TResponse>
}

impl<TPayload, TResponse> Message<TPayload, TResponse> {
    ///
    /// Creates a new message and returns both the message and its channel
    ///
    pub (crate) fn new(message: TPayload) -> (Self, oneshot::Receiver<TResponse>) {
        let (sender, receiver)  = oneshot::channel();
        let message             = Message {
            message:    message,
            response:   sender,
        };

        (message, receiver)
    }

    ///
    /// Returns the result for this message to the sender
    ///
    /// This will return `Err(response)` if nothing is listening for the result of this message
    ///
    pub fn respond(self, response: TResponse) -> Result<(), TResponse> {
        self.response.send(response)
    }

    ///
    /// Responds to the sender and retrieves the payload for the message
    ///
    /// This will return `Err(payload)` if nothing is listening for the response
    ///
    pub fn take(self, response: TResponse) -> Result<TPayload, TPayload> {
        if self.response.send(response).is_err() {
            Err(self.message)
        } else {
            Ok(self.message)
        }
    }
}

impl<TPayload, TResponse> Deref for Message<TPayload, TResponse> {
    type Target = TPayload;

    #[inline]
    fn deref(&self) -> &TPayload {
        &self.message
    }
}

impl<TPayload, TResponse> DerefMut for Message<TPayload, TResponse> {
    #[inline]
    fn deref_mut(&mut self) -> &mut TPayload {
        &mut self.message
    }
}

impl<TPayload, TResponse> fmt::Debug for Message<TPayload, TResponse>
where
    TPayload: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.write_fmt(format_args!("Message({:?})", self.message))
    }
}

impl<TPayload, TResponse> PartialEq for Message<TPayload, TResponse>
where
    TPayload: PartialEq,
{
    fn eq(&self, b: &Self) -> bool {
        self.message.eq(&b.message)
    }
}

impl<TPayload, TResponse> Eq for Message<TPayload, TResponse> where TPayload: Eq {}

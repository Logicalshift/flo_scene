use super::class::*;
use super::context::*;
use super::continuation::*;
use super::message::*;
use super::runtime::*;
use super::value::*;

use futures::prelude::*;
use futures::task::{Poll};

///
/// A reference to the data for a class from the allocator
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TalkDataHandle(pub usize);

///
/// A reference to a data structure within a TalkContext
///
/// FloTalk data is stored by class and handle. References are only valid for the context that they were created for.
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TalkReference(pub (crate) TalkClass, pub (crate) TalkDataHandle);

impl TalkReference {
    ///
    /// Creates a reference from a data handle
    ///
    #[inline]
    pub fn from_handle(class: TalkClass, data_handle: TalkDataHandle) -> TalkReference {
        TalkReference(class, data_handle)
    }

    ///
    /// Increases the reference count for this reference. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn add_reference(&self, context: &mut TalkContext) {
        context.get_callbacks(self.0).add_reference(self.1)
    }

    ///
    /// Decreases the reference count for this reference. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn remove_reference(&self, context: &mut TalkContext) {
        context.get_callbacks(self.0).remove_reference(self.1)
    }

    ///
    /// Sends a message to this object.
    ///
    #[inline]
    pub fn send_message_in_context(&self, message: TalkMessage, context: &mut TalkContext) -> TalkContinuation {
        context.get_callbacks(self.0).send_message(self.1, message)
    }

    ///
    /// Sends a message to this object
    ///
    pub fn send_message(&self, message: TalkMessage, runtime: &TalkRuntime) -> impl Future<Output=TalkValue> {
        let reference                   = *self;
        let mut message                 = Some(message);
        let mut message_continuation    = None;

        runtime.run_continuation(TalkContinuation::Later(Box::new(move |talk_context, future_context| {
            // First, send the message
            if let Some(message) = message.take() {
                message_continuation = Some(reference.send_message_in_context(message, talk_context));
            }

            // Then, wait for the message to complete
            match message_continuation.as_mut().unwrap() {
                TalkContinuation::Ready(value)  => Poll::Ready(value.clone()),
                TalkContinuation::Later(later)  => later(talk_context, future_context),
            }
        })))
    }
}

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
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TalkDataHandle(pub usize);

///
/// A reference to a data structure within a TalkContext
///
/// FloTalk data is stored by class and handle. References are only valid for the context that they were created for.
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
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
    /// This will create a copy of this reference and increase its reference count
    ///
    #[inline]
    pub fn clone_in_context(&self, context: &TalkContext) -> TalkReference {
        let clone = TalkReference(self.0, self.1);
        if let Some(callbacks) = context.get_callbacks(self.0) {
            callbacks.add_reference(self.1);
        }
        clone
    }

    ///
    /// Increases the reference count for this reference. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn add_reference(&self, context: &mut TalkContext) {
        context.get_callbacks_mut(self.0).add_reference(self.1)
    }

    ///
    /// Decreases the reference count for this reference. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn remove_reference(&self, context: &mut TalkContext) {
        context.get_callbacks_mut(self.0).remove_reference(self.1)
    }

    ///
    /// Sends a message to this object.
    ///
    #[inline]
    pub fn send_message_in_context(&self, message: TalkMessage, context: &TalkContext) -> TalkContinuation {
        match context.get_callbacks(self.0) {
            Some(callbacks)     => callbacks.send_message(self.1, message),
            None                => unreachable!("A reference should not reference a class that has not been initialized in the context"),   // As we have to send a message to an instance of a class before we can have a reference to that class, the callbacks should always exist when sending a message to a reference
        }
    }

    ///
    /// Sends a message to this object
    ///
    pub fn send_message_later(&self, message: TalkMessage) -> TalkContinuation {
        let reference                   = *self;
        let mut message                 = Some(message);
        let mut message_continuation    = None;

        TalkContinuation::Later(Box::new(move |talk_context, future_context| {
            loop {
                match message_continuation.take() {
                    None                                    => { message_continuation = Some(reference.send_message_in_context(message.take().unwrap(), talk_context)); },
                    Some(TalkContinuation::Ready(val))      => { message_continuation = Some(TalkContinuation::Ready(TalkValue::Nil)); return Poll::Ready(val); }
                    Some(TalkContinuation::Soon(soon_fn))   => { message_continuation = Some(soon_fn(talk_context)); }
                    Some(TalkContinuation::Later(mut later_fn))   => {
                        let result              = later_fn(talk_context, future_context);
                        message_continuation    = Some(TalkContinuation::Later(later_fn));
                        return result;
                    }
                }
            }
        }))
    }

    ///
    /// Sends a message to this object
    ///
    pub fn send_message(&self, message: TalkMessage, runtime: &TalkRuntime) -> impl Future<Output=TalkValue> {
        runtime.run_continuation(self.send_message_later(message))
    }

    ///
    /// Return the data for a reference cast to a target type (if it can be read as that type)
    ///
    pub fn read_data_in_context<TTargetData>(&self, context: &mut TalkContext) -> Option<TTargetData> 
    where
        TTargetData: 'static,
    {
        context.get_callbacks(self.0).unwrap().read_data(self.1)
    }

    ///
    /// Return the data for a reference cast to a target type (if it can be read as that type)
    ///
    pub fn read_data<'a, TTargetData>(&'a self, runtime: &'a TalkRuntime) -> impl 'a+Future<Output=Option<TTargetData>>
    where
        TTargetData: 'static,
    {
        async move {
            let mut context = runtime.context.lock().await;

            self.read_data_in_context(&mut *context)
        }
    }
}

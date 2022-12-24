use super::class::*;
use super::context::*;
use super::error::*;
use super::message::*;
use super::reference::*;
use super::value::*;

use futures::prelude::*;
use futures::task::{Poll, Context};

use std::mem;

///
/// Raw functions return a continuation, which specifies how a result may be retrieved
///
pub enum TalkContinuation<'a> {
    /// A value that's ready now
    Ready(TalkValue),

    /// A value that requires access to the context to compute, but which doesn't require awaiting a future
    Soon(Box<dyn 'a + Send + FnOnce(&mut TalkContext) -> TalkContinuation<'static>>),

    /// A value that is ready when a future completes
    Later(Box<dyn 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkContinuation<'static>>>),
}

impl<'a> TalkContinuation<'a> {
    ///
    /// Polls this continuation for a result
    ///
    #[inline]
    pub fn poll(&mut self, talk_context: &mut TalkContext, future_context: &mut Context) -> Poll<TalkValue> {
        use TalkContinuation::*;

        loop {
            let mut continuation = TalkContinuation::Ready(TalkValue::Nil);
            mem::swap(&mut continuation, self);

            match continuation {
                Ready(value)        => { return Poll::Ready(value); },
                Soon(soon)          => { *self = soon(talk_context); }

                Later(mut poll_fn)  => { 
                    let result = poll_fn(talk_context, future_context);
                    if let Poll::Ready(next) = result {
                        *self = next;
                    } else {
                        return Poll::Pending;
                    }
                },
            }
        }
    }

    ///
    /// Creates a 'TalkContinuation::Soon' from a function
    ///
    #[inline]
    pub fn soon(soon: impl 'a + Send + FnOnce(&mut TalkContext) -> TalkContinuation<'static>) -> Self {
        TalkContinuation::Soon(Box::new(soon))
    }

    ///
    /// Creates a 'TalkContinuation::Later' from a function returning a value
    ///
    #[inline]
    pub fn later(later: impl 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkValue>) -> Self {
        let mut later = later;
        TalkContinuation::Later(Box::new(move |talk_context, future_context| later(talk_context, future_context).map(|result| TalkContinuation::Ready(result))))
    }

    ///
    /// Creates a 'TalkContinuation::Later' from a function returning a further continuation
    ///
    #[inline]
    pub fn later_soon(later: impl 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkContinuation<'static>>) -> Self {
        TalkContinuation::Later(Box::new(later))
    }

    ///
    /// Creates a TalkContinuation from a future
    ///
    #[inline]
    pub fn future<TFuture>(future: TFuture) -> Self
    where
        TFuture: 'a + Send + Future<Output=TalkValue>,
    {
        let mut future = Box::pin(future);
        Self::later(move |_, ctxt| future.poll_unpin(ctxt))
    }

    ///
    /// Creates a TalkContinuation from a future
    ///
    #[inline]
    pub fn future_soon<TFuture>(future: TFuture) -> Self 
    where
        TFuture: 'a + Send + Future<Output=TalkContinuation<'static>>,
    {
        let mut future              = Box::pin(future);

        Self::later_soon(move |_talk_context, future_context| future.poll_unpin(future_context))
    }

    ///
    /// Once this continuation is finished, perform the specified function on the result
    ///
    #[inline]
    pub fn and_then(self, and_then: impl 'static + Send + FnOnce(TalkValue) -> TalkContinuation<'static>) -> TalkContinuation<'a> {
        match self {
            TalkContinuation::Ready(value)  => and_then(value),
            TalkContinuation::Soon(soon)    => TalkContinuation::Soon(Box::new(move |context| soon(context).and_then(and_then))),

            TalkContinuation::Later(later)  => {
                let mut later       = later;
                let mut and_then    = Some(and_then);

                TalkContinuation::Later(Box::new(move |talk_context, future_context| {
                    // Poll the 'later' value
                    let poll_result = later(talk_context, future_context);

                    if let Poll::Ready(value) = poll_result {
                        if let Some(and_then) = and_then.take() {
                            // When it finishes, call the 'and_then' function and update the 'later' value
                            Poll::Ready(value.and_then(and_then))
                        } else {
                            // Continuation has finished and the value is ready
                            Poll::Ready(value)
                        }
                    } else {
                        // Still pending
                        poll_result
                    }
                }))
            }
        }
    }

    ///
    /// Once this continuation is finished, perform the specified function on the result
    ///
    #[inline]
    pub fn and_then_soon(self, and_then: impl 'static + Send + FnOnce(TalkValue, &mut TalkContext) -> TalkContinuation<'static>) -> TalkContinuation<'a> {
        self.and_then(move |value| TalkContinuation::Soon(Box::new(move |context| and_then(value, context))))
    }

    ///
    /// Creates a continuation that reads the contents of a value (assuming it belongs to the specified allocator)
    ///
    #[inline]
    pub fn read_value<TClass, TOutput>(value: TalkValue, read_value: impl 'a + Send + FnOnce(&mut TClass::Data, &TalkContext) -> TOutput) -> TalkContinuation<'a>
    where
        TClass:     'static + TalkClassDefinition,
        TOutput:    Into<TalkContinuation<'static>>,
    {
        TalkContinuation::Soon(Box::new(move |talk_context| {
            match value {
                TalkValue::Reference(TalkReference(class_id, data_handle)) => {
                    // Get the callbacks for the class
                    let callbacks = talk_context.get_callbacks_mut(class_id);
                    if let Some(allocator) = callbacks.allocator::<TClass::Allocator>() {
                        // Retrieve the value of this data handle
                        let mut allocator   = allocator.lock().unwrap();
                        let data            = allocator.retrieve(data_handle);

                        // Call the callback to read the data
                        read_value(data, talk_context).into()
                    } else {
                        // Not the expected allocator
                        TalkError::UnexpectedClass.into()
                    }
                }

                TalkValue::Error(err)   => err.into(),
                _                       => TalkError::UnexpectedClass.into()
            }
        }))
    }
}

impl<'a, T> From<T> for TalkContinuation<'a>
where
    T : Into<TalkValue>,
{
    #[inline]
    fn from(val: T) -> TalkContinuation<'a> {
        TalkContinuation::Ready(val.into())
    }
}

impl<'a, T, TErr> From<Result<T, TErr>> for TalkContinuation<'a>
where
    T: Into<TalkValue>,
    TErr: Into<TalkError>
{
    #[inline]
    fn from(val: Result<T, TErr>) -> TalkContinuation<'a> {
        match val {
            Ok(val)     => TalkContinuation::Ready(val.into()),
            Err(err)    => TalkContinuation::Ready(TalkValue::Error(err.into()))
        }
    }
}

impl<'a> From<TalkSendMessage> for TalkContinuation<'a> {
    #[inline]
    fn from(TalkSendMessage(target, message): TalkSendMessage) -> TalkContinuation<'a> {
        let mut target                  = target;
        let mut message                 = Some(message);

        TalkContinuation::soon(move |talk_context| target.take().send_message_in_context(message.take().unwrap(), talk_context))
    }
}

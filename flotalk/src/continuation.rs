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
    Later(Box<dyn 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkValue>>),
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
                    *self = TalkContinuation::Later(poll_fn);
                    return result;
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
    /// Creates a 'TalkContinuation::Later' from a function
    ///
    #[inline]
    pub fn later(later: impl 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkValue>) -> Self {
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
    pub fn future_soon<'b: 'a, TFuture>(future: TFuture) -> Self 
    where
        TFuture: 'a + Send + Future<Output=TalkContinuation<'b>>,
    {
        let mut next_continuation   = None;
        let mut future              = Box::pin(future);

        Self::later(move |talk_context, future_context| {
            loop {
                next_continuation = match next_continuation.take() {
                    Some(TalkContinuation::Ready(value))    => { return Poll::Ready(value); },
                    Some(TalkContinuation::Soon(soon_fn))   => { Some((soon_fn)(talk_context)) }
                    Some(TalkContinuation::Later(later_fn)) => {
                        let mut later_fn    = later_fn;
                        let poll_result     = later_fn(talk_context, future_context);
                        next_continuation   = Some(TalkContinuation::Later(later_fn));
                        return poll_result;
                    } 

                    None                                    => {
                        let poll_result = future.poll_unpin(future_context);

                        match poll_result {
                            Poll::Pending                   => { return Poll::Pending; },
                            Poll::Ready(new_continuation)   => Some(new_continuation),
                        }
                    }
                }
            }
        })
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
                let mut later       = TalkContinuation::Later(later);
                let mut and_then    = Some(and_then);

                TalkContinuation::Later(Box::new(move |talk_context, future_context| {
                    // Poll the 'later' value
                    let poll_result = later.poll(talk_context, future_context);

                    if let Poll::Ready(value) = poll_result {
                        if let Some(and_then) = and_then.take() {
                            // When it finishes, call the 'and_then' function and update the 'later' value
                            later       = and_then(value);

                            // Re-poll the new 'later' value and return that as our result
                            later.poll(talk_context, future_context)
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
        let mut message_continuation    = None;

        TalkContinuation::Later(Box::new(move |talk_context, future_context| {
            loop {
                match message_continuation.take() {
                    None                                    => { message_continuation = Some(target.take().send_message_in_context(message.take().unwrap(), talk_context)); },
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
}

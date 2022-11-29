use super::context::*;
use super::error::*;
use super::message::*;
use super::value::*;

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
                    let mut poll_result = later.poll(talk_context, future_context);

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

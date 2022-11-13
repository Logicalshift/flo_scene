use super::context::*;
use super::error::*;
use super::message::*;
use super::value::*;

use futures::task::{Poll, Context};

use std::mem;

///
/// Raw functions return a continuation, which specifies how a result may be retrieved
///
pub enum TalkContinuation {
    /// A value that's ready now
    Ready(TalkValue),

    /// A value that requires access to the context to compute, but which doesn't require awaiting a future
    Soon(Box<dyn Send + FnOnce(&mut TalkContext) -> TalkValue>),

    /// A value that is ready when a future completes
    Later(Box<dyn Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkValue>>),
}

impl TalkContinuation {
    ///
    /// Polls this continuation for a result
    ///
    #[inline]
    pub fn poll(&mut self, talk_context: &mut TalkContext, future_context: &mut Context) -> Poll<TalkValue> {
        use TalkContinuation::*;

        match self {
            Ready(value)    => Poll::Ready(value.clone()),
            Later(poll_fn)  => poll_fn(talk_context, future_context),
            Soon(soon)      => {
                let mut also_soon: Box<dyn Send + FnOnce(&mut TalkContext) -> TalkValue> = Box::new(|_| TalkValue::Nil);
                mem::swap(soon, &mut also_soon);

                Poll::Ready(also_soon(talk_context))
            }
        }
    }
}

impl<T> From<T> for TalkContinuation
where
    T : Into<TalkValue>,
{
    #[inline]
    fn from(val: T) -> TalkContinuation {
        TalkContinuation::Ready(val.into())
    }
}

impl<T, TErr> From<Result<T, TErr>> for TalkContinuation
where
    T: Into<TalkValue>,
    TErr: Into<TalkError>
{
    #[inline]
    fn from(val: Result<T, TErr>) -> TalkContinuation {
        match val {
            Ok(val)     => TalkContinuation::Ready(val.into()),
            Err(err)    => TalkContinuation::Ready(TalkValue::Error(err.into()))
        }
    }
}

impl From<TalkSendMessage> for TalkContinuation {
    #[inline]
    fn from(TalkSendMessage(target, message): TalkSendMessage) -> TalkContinuation {
        let mut message                 = Some(message);
        let mut message_continuation    = None;

        TalkContinuation::Later(Box::new(move |talk_context, future_context| {
            loop {
                match message_continuation.take() {
                    None                                    => { message_continuation = Some(target.send_message_in_context(message.take().unwrap(), talk_context)); },
                    Some(TalkContinuation::Ready(val))      => { message_continuation = Some(TalkContinuation::Ready(TalkValue::Nil)); return Poll::Ready(val); }
                    Some(TalkContinuation::Soon(soon_fn))   => {
                        message_continuation    = Some(TalkContinuation::Ready(TalkValue::Nil));
                        let result              = soon_fn(talk_context);
                        return Poll::Ready(result);
                    }
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

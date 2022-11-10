use super::value::*;
use super::context::*;

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
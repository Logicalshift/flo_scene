use super::context::*;
use super::continuation::*;
use super::value::*;

use futures::prelude::*;
use futures::future;
use futures::lock;
use futures::task::{Poll};

use std::sync::*;

///
/// A `TalkRuntime` is used to run continuations inside a `TalkContext` (it wraps a TalkContext,
/// and schedules continuations on them)
///
pub struct TalkRuntime {
    context: Arc<lock::Mutex<TalkContext>>
}

impl TalkRuntime {
    ///
    /// Creates a runtime for a context
    ///
    pub fn with_context(context: TalkContext) -> TalkRuntime {
        TalkRuntime {
            context: Arc::new(lock::Mutex::new(context))
        }
    }

    ///
    /// Returns an empty runtime
    ///
    pub fn empty() -> TalkRuntime {
        Self::with_context(TalkContext::empty())
    }

    ///
    /// Runs a continuation on this runtime
    ///
    pub fn run_continuation(&self, continuation: TalkContinuation) -> impl Send + Future<Output=TalkValue> {
        let talk_context = Arc::downgrade(&self.context);

        future::poll_fn(move |future_context| {
            Poll::Pending
        })
    }
}

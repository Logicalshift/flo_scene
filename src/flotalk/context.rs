use super::class::*;
use super::continuation::*;
use super::value::*;

use ::desync::*;

use futures::prelude::*;
use futures::future;

use std::sync::*;

///
/// A talk context is a self-contained representation of the state of a flotalk interpreter
///
/// Contexts are only accessed on one thread at a time. They're wrapped by a `TalkRuntime`, which deals with
/// scheduling continuations on a context
///
pub struct TalkContext {
    /// Allocators for this context, indexed by class ID
    context_callbacks: Vec<Option<TalkClassContextCallbacks>>,
}

impl TalkContext {
    ///
    /// Creates a new, empty context
    ///
    pub fn empty() -> TalkContext {
        TalkContext {
            context_callbacks: vec![]
        }
    }

    ///
    /// Creates the allocator for a particular class
    ///
    fn create_callbacks<'a>(&'a mut self, class: TalkClass) -> &'a mut TalkClassContextCallbacks {
        let TalkClass(class_id) = class;

        while self.context_callbacks.len() <= class_id {
            self.context_callbacks.push(None);
        }

        let class_callbacks     = class.callbacks();
        let context_callbacks   = class_callbacks.create_in_context();

        self.context_callbacks[class_id] = Some(context_callbacks);
        self.context_callbacks[class_id].as_mut().unwrap()
    }

    ///
    /// Retrieves the allocator for a class
    ///
    #[inline]
    pub (crate) fn get_callbacks<'a>(&'a mut self, class: TalkClass) -> &'a mut TalkClassContextCallbacks {
        let TalkClass(class_id) = class;

        if self.context_callbacks.len() < class_id {
            if self.context_callbacks[class_id].is_some() {
                return self.context_callbacks[class_id].as_mut().unwrap()
            }
        }

        self.create_callbacks(class)
    }
}

///
/// Runs a continuation to completion in a TalkContext
///
pub fn talk_run_continuation(talk_context: &Arc<Desync<TalkContext>>, continuation: TalkContinuation) -> impl Future<Output=TalkValue> {
    let talk_context = Arc::clone(talk_context);
    let (now, later) = match continuation {
        TalkContinuation::Ready(now)    => (Some(now), None),
        TalkContinuation::Later(later)  => (None, Some(later)),
    };

    async move {
        // Return immediately if the continuation is already complete
        if let Some(now) = now { return now; }

        // Run the continuation in the context
        if let Some(mut later) = later {
            future::poll_fn(move |mut future_context| {
                let later = &mut later;

                // Poll the 'later' future in the context of the talk_context (TODO: could use future_sync here instead of just sync)
                talk_context.sync(move |talk_context| {
                    later(talk_context, &mut future_context)
                })
            }).await
        } else {
            // Either 'now' or 'later' must return a value
            unreachable!()
        }
    }
}

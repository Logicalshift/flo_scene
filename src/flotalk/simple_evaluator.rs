use super::continuation::*;
use super::context::*;
use super::instruction::*;
use super::symbol::*;
use super::value::*;
use super::value_store::*;

use futures::prelude::*;
use futures::task::{Poll};

use std::sync::*;

enum TalkWaitState {
    // Evalulate the next expression
    Run,

    /// Wait for the specified future to complete, then push the result to the stack
    WaitFor(TalkContinuation),

    /// Expression completed, returned a value
    Finished(TalkValue),
}

struct TalkStack {
    /// Program counter
    pc: usize,

    /// Value stack
    stack: Vec<TalkValue>,

    /// Symbol stores
    symbol_store: Vec<TalkValueStore<TalkValue>>,
}

///
/// Evaluates expressions from a particular point (until we have a single result or we hit a future)
///
#[inline]
fn eval_at(root_values: Arc<Mutex<TalkValueStore<TalkValue>>>, expression: Arc<Vec<TalkInstruction<impl 'static + Send + TryInto<TalkValue>, impl 'static + Send + Into<TalkSymbol>>>>, stack: &mut TalkStack, context: &mut TalkContext) -> TalkWaitState {
    let mut stack = stack;

    todo!()
}

///
/// Evaluates a FloTalk expression which does not have any binding specified, and where Literals have not been parsed into values
///
/// This is the simplest form of expression evaluator, which runs the slowest out of all the possible implementations (due to needing to parse values and look up
/// symbols every time)
///
pub fn talk_evaluate_simple(root_values: Arc<Mutex<TalkValueStore<TalkValue>>>, expression: Arc<Vec<TalkInstruction<impl 'static + Sync + Send + TryInto<TalkValue>, impl 'static + Sync + Send + Into<TalkSymbol>>>>) -> TalkContinuation {
    let mut wait_state = TalkWaitState::Run;
    let mut stack       = TalkStack { pc: 0, stack: vec![], symbol_store: vec![TalkValueStore::default()] };

    TalkContinuation::Later(Box::new(move |talk_context, future_context| {
        use TalkWaitState::*;

        // Poll the future if we're in an appropriate state
        if let WaitFor(future) = &mut wait_state {
            // If ready, push the result and move to the 'run' state
            if let Poll::Ready(value) = future.poll(talk_context, future_context) {
                stack.stack.push(value);
                wait_state = Run;
            }
        }

        // Run until the future futures
        while let Run = &wait_state {
            // Evaluate until we hit a point where we are finished or need to poll a future
            wait_state = eval_at(Arc::clone(&root_values), Arc::clone(&expression), &mut stack, talk_context);

            // Poll the future if one is returned
            if let WaitFor(future) = &mut wait_state {
                // If ready, push the result and move to the 'run' state
                if let Poll::Ready(value) = future.poll(talk_context, future_context) {
                    stack.stack.push(value);
                    wait_state = Run;
                }
            }
        }

        // Return the value if finished
        match &wait_state {
            WaitFor(future) => Poll::Pending,
            Run             => Poll::Pending,
            Finished(value) => Poll::Ready(value.clone()),
        }
    }))
}

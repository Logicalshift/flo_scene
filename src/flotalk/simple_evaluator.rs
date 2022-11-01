use super::continuation::*;
use super::context::*;
use super::error::*;
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
fn eval_at<TValue, TSymbol>(root_values: &mut TalkValueStore<TalkValue>, expression: &Vec<TalkInstruction<TValue, TSymbol>>, stack: &mut TalkStack, context: &mut TalkContext) -> TalkWaitState 
where
    TValue:     'static,
    TSymbol:    'static,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    // Set up the evaluation
    let mut stack       = stack;
    let expression_len  = expression.len();

    loop {
        // If the PC has passed beyond the end of the expression, we're finished
        if stack.pc > expression_len {
            return TalkWaitState::Finished(stack.stack.pop().unwrap_or(TalkValue::Nil));
        }

        // Fetch the next instruction and move the program counter on
        let next_instruction = &expression[stack.pc];
        stack.pc += 1;

        // Execute the instruction
        use TalkInstruction::*;

        match next_instruction {
            // Follow code comes from the specified location
            Location(_location) => {}

            // Creates (or replaces) a local binding location for a symbol
            PushLocalBinding(symbol) => {
                todo!()
            }

            // Restores the previous binding for the specified symbol
            PopLocalBinding(symbol) => {
                todo!()
            }

            // Load the value indicating 'nil' to the stack
            LoadNil => {
                stack.stack.push(TalkValue::Nil);
            }

            // Load a literal value onto the stack
            Load(literal) => {
                match literal.try_into() {
                    Ok(value)   => stack.stack.push(value),
                    Err(err)    => return TalkWaitState::Finished(TalkValue::Error(err)),
                }
            }

            // Load a symbol value onto the stack
            LoadFromSymbol(symbol) => {
                let symbol = TalkSymbol::from(symbol);

                todo!()
            }

            // Load an object representing a code block onto the stack
            LoadBlock(variables, instructions) => {
                todo!()
            }

            // Loads the value from the top of the stack and stores it a variable
            StoreAtSymbol(symbol) => {
                todo!()
            }

            // Pops an object off the stack and sends the specified message
            SendUnaryMessage(symbol) => {
                todo!()
            }

            // Pops message arguments and an object from the stack, and sends the specified message, leaving the result on the stack. Number of arguments is supplied, and must match the number in the message signature.
            SendMessage(message_id, arg_count) => {
                todo!()
            }

            // Copies the value on top of the stack
            Duplicate => {
                let val = stack.stack.pop().unwrap();

                stack.stack.push(val.clone());
                stack.stack.push(val);
            }

            // Discards the value on top of the stack
            Discard => {
                stack.stack.pop();
            }
        }

    }

    todo!()
}

///
/// Evaluates a FloTalk expression which does not have any binding specified, and where Literals have not been parsed into values
///
/// This is the simplest form of expression evaluator, which runs the slowest out of all the possible implementations (due to needing to parse values and look up
/// symbols every time)
///
pub fn talk_evaluate_simple<TValue, TSymbol>(root_values: Arc<Mutex<TalkValueStore<TalkValue>>>, expression: Arc<Vec<TalkInstruction<TValue, TSymbol>>>) -> TalkContinuation 
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
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
            let mut root_values = root_values.lock().unwrap();
            wait_state = eval_at(&mut *root_values, &*expression, &mut stack, talk_context);

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

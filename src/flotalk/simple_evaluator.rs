use super::continuation::*;
use super::context::*;
use super::error::*;
use super::instruction::*;
use super::message::*;
use super::releasable::*;
use super::simple_evaluator_block::*;
use super::symbol::*;
use super::value::*;
use super::value_store::*;

use futures::task::{Poll};

use std::sync::*;
use std::collections::{HashMap};

enum TalkWaitState {
    // Evalulate the next expression
    Run,

    /// Wait for the specified future to complete, then push the result to the stack
    WaitFor(TalkContinuation<'static>),

    /// Expression completed, returned a value
    Finished(TalkValue),
}

///
/// A call frame
///
struct TalkFrame {
    /// Program counter
    pc: usize,

    /// Value stack
    stack: Vec<TalkValue>,

    /// Symbols defined in the outer contexts (lower accessed first)
    outer_bindings: Vec<Arc<Mutex<TalkValueStore<TalkValue>>>>,

    /// Symbols defined in the current context
    local_bindings: TalkValueStore<TalkValue>,

    /// The earlier binding locations for symbols, when popped using PopLocalBinding
    earlier_bindings: HashMap<TalkSymbol, Vec<(i32, usize)>>,
}

impl TalkFrame {
    ///
    /// Performs an action on the value of a symbol
    ///
    #[inline]
    pub fn with_symbol_value<TResult>(&mut self, symbol: TalkSymbol, action: impl FnOnce(&mut TalkValue) -> TResult) -> Option<TResult> {
        if let Some(value) = self.local_bindings.value_for_symbol(symbol) {
            // In the local binding
            Some(action(value))
        } else {
            // Check the outer bindings
            for store in self.outer_bindings.iter() {
                let mut store = store.lock().unwrap();

                if let Some(value) = store.value_for_symbol(symbol) {
                    return Some(action(value));
                }
            }

            None
        }
    }

    ///
    /// Stores the current value of a binding in the list of earlier bindings
    ///
    #[inline]
    pub fn push_binding(&mut self, symbol: TalkSymbol) {
        // Store the previous value for this symbol
        if let Some(loc) = self.local_bindings.location_for_symbol(symbol) {
            // In the local binding
            self.earlier_bindings.entry(symbol)
                .or_insert_with(|| vec![])
                .push((-1, loc));
        } else {
            // Check the outer bindings
            for pos in 0..self.outer_bindings.len() {
                if let Some(loc) = self.outer_bindings[pos].lock().unwrap().location_for_symbol(symbol) {
                    self.earlier_bindings.entry(symbol)
                        .or_insert_with(|| vec![])
                        .push((pos as i32, loc));

                    break;
                }
            }
        }

        // Create a value in the local binding
        self.local_bindings.define_symbol(symbol);
    }

    ///
    /// Removes the binding from the list of earlier bindings and restores it to its previous value
    ///
    /// (We assume that any replacement binding was created in the local bindings)
    ///
    #[inline]
    pub fn pop_binding(&mut self, symbol: TalkSymbol) {
        // Fetch the last binding position
        let (last_pos, last_loc) = self.earlier_bindings.get_mut(&symbol).unwrap().pop().unwrap();

        if last_pos == -1 {
            self.local_bindings.set_symbol_location(symbol, last_loc);
        } else {
            self.local_bindings.undefine_symbol(symbol);
        }
    }

    ///
    /// Release all the references in this frame
    ///
    pub fn remove_all_references(&mut self, context: &mut TalkContext) {
        // Free the stack values
        while let Some(val) = self.stack.pop() {
            val.remove_reference(context);
        }

        // Free anything in the local bindings
        self.local_bindings.remove_all_references(context);
    }
}

///
/// Evaluates expressions from a particular point (until we have a single result or we hit a future)
///
#[inline]
fn eval_at<TValue, TSymbol>(expression: &Vec<TalkInstruction<TValue, TSymbol>>, frame: &mut TalkFrame, context: &mut TalkContext) -> TalkWaitState 
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    // Set up the evaluation
    let mut frame       = frame;
    let expression_len  = expression.len();

    loop {
        // If the PC has passed beyond the end of the expression, we're finished
        if frame.pc >= expression_len {
            return TalkWaitState::Finished(frame.stack.pop().unwrap_or(TalkValue::Nil));
        }

        // Fetch the next instruction and move the program counter on
        let next_instruction = &expression[frame.pc];
        frame.pc += 1;

        // Execute the instruction
        use TalkInstruction::*;

        match next_instruction {
            // Follow code comes from the specified location
            Location(_location) => {}

            // Creates (or replaces) a local binding location for a symbol
            PushLocalBinding(symbol) => {
                frame.push_binding(symbol.into());
            }

            // Restores the previous binding for the specified symbol
            PopLocalBinding(symbol) => {
                frame.pop_binding(symbol.into());
            }

            // Load the value indicating 'nil' to the stack
            LoadNil => {
                frame.stack.push(TalkValue::Nil);
            }

            // Load a literal value onto the stack
            Load(literal) => {
                match literal.try_into() {
                    Ok(value)   => frame.stack.push(value),
                    Err(err)    => return TalkWaitState::Finished(TalkValue::Error(err)),
                }
            }

            // Load a symbol value onto the stack
            LoadFromSymbol(symbol) => {
                let symbol = TalkSymbol::from(symbol);

                if let Some(value) = frame.with_symbol_value(symbol, |value| value.clone_in_context(context)) {
                    frame.stack.push(value);
                } else {
                    return TalkWaitState::Finished(TalkValue::Error(TalkError::UnboundSymbol(symbol)));
                }
            }

            // Load an object representing a code block onto the stack
            LoadBlock(variables, instructions) => {
                // TODO: closure binding
                // TODO: even for the simple evaluator this is really too slow, add an optimiser that pre-binds the blocks

                // Create the block, and add it to the stack
                let block_reference = simple_evaluator_block(context, variables.clone(), frame.outer_bindings.clone(), Arc::clone(instructions));
                frame.stack.push(TalkValue::Reference(block_reference));
            }

            // Loads the value from the top of the stack and stores it a variable
            StoreAtSymbol(symbol) => {
                let new_value   = frame.stack.pop().unwrap();
                let symbol      = TalkSymbol::from(symbol);

                if let Some(()) = frame.with_symbol_value(symbol, move |value| *value = new_value) {
                    // Value stored
                } else {
                    // TODO: declare in the outer state?
                    return TalkWaitState::Finished(TalkValue::Error(TalkError::UnboundSymbol(symbol)));
                }
            }

            // Pops message arguments and an object from the stack, and sends the specified message, leaving the result on the stack. Number of arguments is supplied, and must match the number in the message signature.
            SendMessage(message_id, arg_count) => {
                // TODO: need to handle releasing arguments after the message has been completed
                //      (better if the receiver is responsible for releasing its arguments and itself...)

                // Pop arguments
                let mut args = smallvec![];
                for _ in 0..*arg_count {
                    args.push(frame.stack.pop().unwrap());
                }

                // Pop the target
                let target = frame.stack.pop().unwrap();

                // Generate the message
                let message = if *arg_count == 0 { TalkMessage::Unary(*message_id) } else { TalkMessage::WithArguments(*message_id, args) };

                // Send it
                let mut continuation = target.send_message_in_context(message, context);

                // Push the result if it's immediately ready, otherwise return a continuation
                loop {
                    match continuation {
                        TalkContinuation::Ready(TalkValue::Error(err))  => return TalkWaitState::Finished(TalkValue::Error(err)),
                        TalkContinuation::Ready(value)                  => { frame.stack.push(value); break; },
                        TalkContinuation::Soon(soon_value)              => { continuation = soon_value(context); }
                        TalkContinuation::Later(later)                  => return TalkWaitState::WaitFor(TalkContinuation::Later(later)),
                    }
                }
            }

            // Copies the value on top of the stack
            Duplicate => {
                let val = frame.stack.pop().unwrap();

                frame.stack.push(val.clone_in_context(context));
                frame.stack.push(val);
            }

            // Discards the value on top of the stack
            Discard => {
                if let Some(old_value) = frame.stack.pop() {
                    old_value.remove_reference(context);
                }
            }
        }
    }
}

///
/// Evaluates a FloTalk expression which does not have any binding specified, and where Literals have not been parsed into values
///
/// This is the simplest form of expression evaluator, which runs the slowest out of all the possible implementations (due to needing to parse values and look up
/// symbols every time)
///
pub fn talk_evaluate_simple<TValue, TSymbol>(root_values: Vec<Arc<Mutex<TalkValueStore<TalkValue>>>>, expression: Arc<Vec<TalkInstruction<TValue, TSymbol>>>) -> TalkContinuation<'static> 
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    talk_evaluate_simple_with_arguments(root_values, TalkValueStore::default(), expression)
}

///
/// Evaluates a FloTalk expression which does not have any binding specified, and where Literals have not been parsed into values
///
/// This is the simplest form of expression evaluator, which runs the slowest out of all the possible implementations (due to needing to parse values and look up
/// symbols every time)
///
pub fn talk_evaluate_simple_with_arguments<TValue, TSymbol>(root_values: Vec<Arc<Mutex<TalkValueStore<TalkValue>>>>, arguments: TalkValueStore<TalkValue>, expression: Arc<Vec<TalkInstruction<TValue, TSymbol>>>) -> TalkContinuation<'static> 
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    let mut wait_state  = TalkWaitState::Run;
    let mut frame       = TalkFrame { pc: 0, stack: vec![], outer_bindings: root_values, local_bindings: arguments, earlier_bindings: HashMap::new() };

    TalkContinuation::Later(Box::new(move |talk_context, future_context| {
        use TalkWaitState::*;

        // Poll the future if we're in an appropriate state
        if let WaitFor(future) = &mut wait_state {
            // If ready, push the result and move to the 'run' state
            if let Poll::Ready(value) = future.poll(talk_context, future_context) {
                if let TalkValue::Error(err) = value {
                    // Errors abort the rest of the evaluation and are returned directly
                    wait_state = Finished(TalkValue::Error(err));
                } else {
                    // Future is finished: push the new value to the stack and continue
                    frame.stack.push(value);
                    wait_state = Run;
                }
            }
        }

        // Run until the future futures
        while let Run = &wait_state {
            // Evaluate until we hit a point where we are finished or need to poll a future
            wait_state = eval_at(&*expression, &mut frame, talk_context);

            // Poll the future if one is returned
            if let WaitFor(future) = &mut wait_state {
                // If ready, push the result and move to the 'run' state
                if let Poll::Ready(value) = future.poll(talk_context, future_context) {
                    frame.stack.push(value);
                    wait_state = Run;
                }
            }
        }

        // Return the value if finished
        match &mut wait_state {
            WaitFor(_)      => Poll::Pending,
            Run             => Poll::Pending,
            Finished(value) => {
                frame.remove_all_references(talk_context);
                Poll::Ready(value.take())
            },
        }
    }))
}

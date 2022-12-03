use super::continuation::*;
use super::context::*;
use super::error::*;
use super::instruction::*;
use super::message::*;
use super::releasable::*;
use super::reference::*;
use super::standard_classes::*;
use super::symbol::*;
use super::symbol_table::*;
use super::value::*;

use futures::task::{Poll};
use smallvec::*;

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

    /// The arguments for this call, if they're not loaded yet
    arguments: Option<SmallVec<[TalkValue; 4]>>,

    /// Symbols defined in the outer contexts (lower accessed first)
    bindings: Vec<TalkCellBlock>,

    /// The symbol table for this frame
    symbol_table: TalkSymbolTable,

    /// The earlier binding locations for symbols, when popped using PopLocalBinding
    earlier_bindings: HashMap<TalkSymbol, Vec<TalkFrameCell>>,
}

impl TalkFrame {
    ///
    /// Performs an action on the value of a symbol
    ///
    #[inline]
    pub fn with_symbol_value<TResult>(&mut self, symbol: TalkSymbol, context: &mut TalkContext, action: impl FnOnce(&mut TalkValue) -> TResult) -> Option<TResult> {
        if let Some(binding) = self.symbol_table.symbol(symbol) {
            let cell_block  = self.bindings[binding.frame as usize];
            let cell_block  = context.cell_block_mut(cell_block);

            Some(action(&mut cell_block[binding.cell as usize]))
        } else {
            None
        }
    }

    ///
    /// Stores the current value of a binding in the list of earlier bindings
    ///
    #[inline]
    pub fn push_binding(&mut self, symbol: TalkSymbol, context: &mut TalkContext) {
        if let Some(old_location) = self.symbol_table.symbol(symbol) {
            // Store the old location for the binding
            self.earlier_bindings.entry(symbol)
                .or_insert_with(|| vec![])
                .push(old_location);
        }

        // Create a new location for this symbol
        let new_symbol = self.symbol_table.define_symbol(symbol);

        // Expand the list of cells if needed
        let cells = context.cell_block(self.bindings[0]);
        if cells.len() <= new_symbol.cell as _ {
            // Reserve space by doubling what we have
            let mut new_len = cells.len();
            while new_len <= new_symbol.cell as _ {
                if new_len < 1 { new_len = 1; }
                new_len *= 2;
            }

            // Resize the cell block
            context.resize_cell_block(self.bindings[0], new_len);
        }
    }

    ///
    /// Removes the binding from the list of earlier bindings and restores it to its previous value
    ///
    /// (We assume that any replacement binding was created in the local bindings)
    ///
    #[inline]
    pub fn pop_binding(&mut self, symbol: TalkSymbol) {
        // Fetch the last binding position
        let last_binding = self.earlier_bindings.get_mut(&symbol).unwrap().pop().unwrap();

        // Undefine the symbol
        self.symbol_table.undefine_symbol(symbol);

        if last_binding.frame == 0 {
            // The previous binding was in the same frame
            self.symbol_table.alias_symbol(symbol, last_binding.cell);
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

        // Free anything in the local bindings (if the arguments haven't been taken yet, there are no local bindings)
        if self.arguments.is_none() {
            context.release_cell_block(self.bindings[0]);
        }
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

    // If the arguments (local frame) have not been allocated yet, allocate them
    if let Some(arguments) = frame.arguments.take() {
        // Create a cell block to store the arguments
        let local_cell_block = context.allocate_cell_block(arguments.len());

        // Move the arguments into the cell block
        let cell_block_values = context.cell_block_mut(local_cell_block);
        for (idx, arg) in arguments.into_iter().enumerate() {
            cell_block_values[idx] = arg;
        }

        // This block becomes the first binding
        frame.bindings.insert(0, local_cell_block);
    }

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
                frame.push_binding(symbol.into(), context);
            }

            // Restores the previous binding for the specified symbol
            PopLocalBinding(symbol) => {
                frame.pop_binding(symbol.into());
            }

            // Sets the symbol values for the arguments for this expression
            LoadArguments(argument_symbols) => {
                // Set up the symbol table by defining each argument in turn (will allocate from 0 if the table is empty)
                debug_assert!(frame.symbol_table.len() == 0);
                for arg_symbol in argument_symbols {
                    frame.symbol_table.define_symbol(arg_symbol);
                }
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

                if let Some(value) = frame.with_symbol_value(symbol, context, |value| value.clone()) {
                    let value = value.clone_in_context(context);
                    frame.stack.push(value);
                } else {
                    return TalkWaitState::Finished(TalkValue::Error(TalkError::UnboundSymbol(symbol)));
                }
            }

            // Load an object representing a code block onto the stack
            LoadBlock(variables, instructions) => {
                // TODO: even for the simple evaluator this is really too slow, add an optimiser that pre-binds the blocks

                // Retain the bindings for the block
                let bindings = frame.bindings.clone();
                for cell_block in bindings.iter() {
                    context.retain_cell_block(*cell_block);
                }

                // Create the block, and add it to the stack
                let block_reference = create_simple_evaluator_block_in_context(context, variables.clone(), bindings, Arc::new(Mutex::new(frame.symbol_table.clone())), Arc::clone(instructions), None);
                frame.stack.push(TalkValue::Reference(block_reference));
            }

            // Loads the value from the top of the stack and stores it a variable
            StoreAtSymbol(symbol) => {
                let new_value   = frame.stack.pop().unwrap();
                let symbol      = TalkSymbol::from(symbol);

                if let Some(()) = frame.with_symbol_value(symbol, context, move |value| *value = new_value) {
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
pub fn talk_evaluate_simple<TValue, TSymbol>(parent_symbol_table: Arc<Mutex<TalkSymbolTable>>, parent_frames: Vec<TalkCellBlock>, expression: Arc<Vec<TalkInstruction<TValue, TSymbol>>>) -> TalkContinuation<'static> 
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    talk_evaluate_simple_with_arguments(parent_symbol_table, parent_frames, smallvec![], expression)
}

///
/// Evaluates a FloTalk expression which does not have any binding specified, and where Literals have not been parsed into values
///
/// This is the simplest form of expression evaluator, which runs the slowest out of all the possible implementations (due to needing to parse values and look up
/// symbols every time)
///
/// The argument cell block will be released when this returns, but the parent frames will not be released (ie, the parent frames are considered borrowed and the arguments are
/// considered owned).
///
pub fn talk_evaluate_simple_with_arguments<TValue, TSymbol>(parent_symbol_table: Arc<Mutex<TalkSymbolTable>>, parent_frames: Vec<TalkCellBlock>, arguments: SmallVec<[TalkValue; 4]>, expression: Arc<Vec<TalkInstruction<TValue, TSymbol>>>) -> TalkContinuation<'static> 
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    // Create the first frame
    let mut wait_state      = TalkWaitState::Run;
    let symbol_table        = TalkSymbolTable::with_parent_frame(parent_symbol_table);      // TODO: makes the cells invalid (frame number is wrong) until LoadArguments is called, which is kind of janky
    let bindings            = parent_frames;
    let mut frame           = TalkFrame { pc: 0, stack: vec![], arguments: Some(arguments), bindings: bindings, symbol_table: symbol_table, earlier_bindings: HashMap::new() };

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

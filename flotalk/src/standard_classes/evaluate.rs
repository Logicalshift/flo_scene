use crate::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::sync::*;

pub (crate) static EVALUATE_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkEvaluateClass));

///
/// The `Evaluate` flotalk class
///
/// This is used to evaluate statements and return the result. It has two main ways it can be used:
///
/// ```smalltalk
/// Evaluate statement: 'example statement'
/// ```
///
/// and
///
/// ```smalltalk
/// Evaluate createBlock: 'example statement'
/// ```
///
/// The first version will evaluate the statement immediately, and the second will return a block which will evaluate the statement whenever
/// the `value` message is sent to it.
///
pub struct TalkEvaluateClass;

impl TalkClassDefinition for TalkEvaluateClass {
    /// The type of the data stored by an object of this class
    type Data = ();

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkStandardAllocator<Self::Data>;

    ///
    /// Creates the allocator for this class in a particular context
    ///
    /// This is also an opportunity for a class to perform any other initialization it needs to do within a particular `TalkContext`
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        Self::Allocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        static MSG_STATEMENT: Lazy<TalkMessageSignatureId>      = Lazy::new(|| "statement:".into());
        static MSG_CREATE_BLOCK: Lazy<TalkMessageSignatureId>   = Lazy::new(|| "createBlock:".into());

        if message_id == *MSG_STATEMENT {
            let statement = args[0].try_as_string();

            match statement {
                Err(err)        => err.into(),
                Ok(statement)   => continuation_from_script(statement),
            }
        } else if message_id == *MSG_CREATE_BLOCK {
            // Fetch the statement
            let statement = args[0].try_as_string();
            let statement = match statement {
                Err(err)        => { return err.into(); },
                Ok(statement)   => statement,
            };

            TalkContinuation::future_soon(async move {
                // Parse the statement to instructions
                let statement = TalkScript::from(statement).to_instructions().await;
                let statement = match statement {
                    Err(err)        => { return TalkError::from(err).into(); }
                    Ok(statement)   => statement,
                };

                // Create a simple evaluator block
                TalkContinuation::soon(move |talk_context| {
                    let statement = Arc::new(statement);

                    // Run with the root symbol table
                    let root_symbol_table = talk_context.root_symbol_table();
                    let root_symbol_block = talk_context.root_symbol_table_cell_block().leak();
                    talk_context.retain_cell_block(root_symbol_block);

                    // Any new symbols are local to the evaluation
                    let eval_symbol_table = Arc::new(Mutex::new(TalkSymbolTable::with_parent_frame(root_symbol_table)));
                    let eval_symbol_block = talk_context.allocate_cell_block(1);

                    create_simple_evaluator_block_in_context(talk_context, vec![], vec![eval_symbol_block, root_symbol_block], eval_symbol_table, statement, None).into()
                })
            })
        } else {
            TalkError::MessageNotSupported(message_id).into()
        }
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _reference: TalkReference, _allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}

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
/// Instances of the `Evaluate` class can be used to evaluate statements using a copy of the current root namespace, for example:
///
/// ```smalltalk
/// | eval |
/// eval := Evaluate new.
/// eval define: #'test' as: 10.
/// eval statement: 'test'
/// ```
///
/// `Evaluate new` will copy the root namespace of the current context (forming an independent root namespace) and `Evaluate newEmpty` will
/// create an evaluator with an empty root namespace. The `define:` call can be used to define values in the root namespace of the evaluator.
///
pub struct TalkEvaluateClass;

///
/// Evaluate instance data
///
pub struct TalkEvaluate {
    /// The cell block containing the values for the root symbol table
    root_cell_block: TalkCellBlock,

    /// The 'root' symbol table, which can be used for binding symbols when they have no symbol table of their own
    root_symbol_table: Arc<Mutex<TalkSymbolTable>>,
}

impl TalkReleasable for TalkEvaluate {
    fn release_in_context(self, context: &TalkContext) {
        self.root_cell_block.release_in_context(context);
    }
}

impl TalkClassDefinition for TalkEvaluateClass {
    /// The type of the data stored by an object of this class
    type Data = TalkEvaluate;

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
    fn send_class_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, class_id: TalkClass, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
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

                    // Any new symbols are local to the evaluation
                    let eval_symbol_table = Arc::new(Mutex::new(TalkSymbolTable::with_parent_frame(root_symbol_table)));
                    let eval_symbol_block = talk_context.allocate_cell_block(1);

                    create_simple_evaluator_block_in_context(talk_context, vec![], vec![eval_symbol_block, root_symbol_block], eval_symbol_table, statement, None).into()
                })
            })

        } else if message_id == *TALK_MSG_NEW {

            let allocator       = Arc::clone(allocator);

            // Clone the symbol table
            let symbol_table    = (*args.context().root_symbol_table().lock().unwrap()).clone();
            let symbol_table    = Arc::new(Mutex::new(symbol_table));

            // Clone the cells in the root
            let root_cells      = args.context().cell_block(&*args.context().root_symbol_table_cell_block());
            let mut root_cells  = root_cells.iter().map(|cell| cell.clone_in_context(args.context())).collect::<Vec<_>>();

            TalkContinuation::soon(move |talk_context| {
                // Create a new cell block
                let cell_block  = talk_context.allocate_cell_block(root_cells.len());

                // Copy in the root cells
                let cells       = talk_context.cell_block_mut(&cell_block);
                for (idx, val) in root_cells.drain(..).enumerate() {
                    cells[idx] = val;
                }

                // Create the evaluator object
                let evaluate = TalkEvaluate {
                    root_cell_block:    cell_block,
                    root_symbol_table:  symbol_table
                };

                // Store in the allocator
                let data_handle = allocator.lock().unwrap().store(evaluate);
                let reference   = TalkReference(class_id, data_handle);

                reference.into()
            })

        } else {
            TalkError::MessageNotSupported(message_id).into()
        }
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        static MSG_STATEMENT: Lazy<TalkMessageSignatureId>      = Lazy::new(|| "statement:".into());
        static MSG_CREATE_BLOCK: Lazy<TalkMessageSignatureId>   = Lazy::new(|| "createBlock:".into());

        let data_handle = reference.data_handle();

        if message_id == *MSG_STATEMENT {

            let statement = args[0].try_as_string();

            match statement {
                Err(err)        => err.into(),
                Ok(statement)   => continuation_from_script(statement),
            }

        } else if message_id == *MSG_CREATE_BLOCK {

            let allocator = Arc::clone(allocator);

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

                    // Read the symbol tables from the instance
                    let (root_symbol_table, root_symbol_block) = allocator.lock().unwrap().retrieve(data_handle).symbol_tables(talk_context);

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
}

impl TalkEvaluate {
    ///
    /// Read & retain the symbol table from this evaluate instance
    ///
    fn symbol_tables(&self, talk_context: &TalkContext) -> (Arc<Mutex<TalkSymbolTable>>, TalkCellBlock) {
        let symbol_table    = Arc::clone(&self.root_symbol_table);
        let cell_block      = self.root_cell_block;
        talk_context.retain_cell_block(cell_block);

        (symbol_table, cell_block)
    }
}

use super::talk_message_handler::*;

use crate::allocator::*;
use crate::class::*;
use crate::context::*;
use crate::continuation::*;
use crate::error::*;
use crate::instruction::*;
use crate::message::*;
use crate::symbol::*;
use crate::reference::*;
use crate::releasable::*;
use crate::simple_evaluator::*;
use crate::symbol_table::*;
use crate::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::any::{TypeId};
use std::collections::{HashMap};
use std::marker::{PhantomData};
use std::sync::*;

/// Maps the type IDs of the value and symbol type to the TalkClass that implements the SimpleEvaluatorClass for that ID type
static SIMPLE_EVALUATOR_CLASS: Lazy<Mutex<HashMap<(TypeId, TypeId), TalkClass>>> = Lazy::new(|| Mutex::new(HashMap::new()));

static VALUE_SYMBOL: Lazy<TalkSymbol>       = Lazy::new(|| TalkSymbol::from("value"));
static VALUE_COLON_SYMBOL: Lazy<TalkSymbol> = Lazy::new(|| TalkSymbol::from("value:"));

///
/// Class that represents a block evaluated by the simple evaluator
///
struct SimpleEvaluatorBlockClass<TValue, TSymbol> 
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    value: PhantomData<TValue>,
    symbol: PhantomData<TSymbol>,
}

///
/// Data storage type for the simple evaluator block class
///
struct SimpleEvaluatorBlock<TValue, TSymbol>
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    /// The ID of the message that evaluates this block
    accepted_message_id:    TalkMessageSignatureId,

    /// The symbol table for the parent frames
    parent_symbol_table:    Arc<Mutex<TalkSymbolTable>>,

    /// The cell blocks representing the parent frames
    parent_frames:          Vec<TalkCellBlock>,

    /// A cell block containing the resources needed by this block (eg, data that it needs to load into the stack)
    resources:              Option<TalkCellBlock>,

    /// The expression to evaluate for this block
    expression:             Arc<Vec<TalkInstruction<TValue, TSymbol>>>,
}

impl<TValue, TSymbol> TalkReleasable for SimpleEvaluatorBlock<TValue, TSymbol>
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    fn release_in_context(mut self, context: &TalkContext) {
        // Release any cells referenced by this evaluator block
        for cell_block in self.parent_frames.iter() {
            context.release_cell_block(*cell_block);
        }

        if let Some(resources) = self.resources.take() {
            context.release_cell_block(resources);
        }
    }
}

impl<TValue, TSymbol> TalkClassDefinition for SimpleEvaluatorBlockClass<TValue, TSymbol>
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    type Data       = SimpleEvaluatorBlock<TValue, TSymbol>;
    type Allocator  = TalkStandardAllocator<Self::Data>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self) -> Self::Allocator {
        TalkStandardAllocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _arguments: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message_id)))
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, arguments: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _reference: TalkReference, target: &mut Self::Data) -> TalkContinuation<'static> {
        if message_id == target.accepted_message_id {
            // Leak the arguments to the method call (it will dispose them when done)
            talk_evaluate_simple_with_arguments(Arc::clone(&target.parent_symbol_table), target.parent_frames.clone(), arguments.leak(), Arc::clone(&target.expression))
        } else {
            // Not the message this block was expecting
            TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message_id)))
        }
    }
}

///
/// Retrieves (or creates) the TalkClass corresponding to a simple evaluator block using the specified value and symbol types for the instructions
///
pub (crate) fn simple_evaluator_block_class<TValue, TSymbol>() -> TalkClass
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    let mut classes     = SIMPLE_EVALUATOR_CLASS.lock().unwrap();
    let evaluator_type  = (TypeId::of::<TValue>(), TypeId::of::<TSymbol>());

    if let Some(class) = classes.get(&evaluator_type) {
        *class
    } else {
        let class = TalkClass::create(SimpleEvaluatorBlockClass { value: PhantomData::<TValue>, symbol: PhantomData::<TSymbol> });

        talk_add_class_data_reader::<SimpleEvaluatorBlockClass<TValue, TSymbol>, TalkClassMessageHandler>(
            |block| {
                // Clone the properties for this message handler
                let parent_symbol_table = block.parent_symbol_table.clone();
                let parent_frames       = block.parent_frames.clone();          // TODO: add/remove references to the frame cells
                let expression          = block.expression.clone();

                TalkClassMessageHandler {
                    define_in_dispatch_table: Box::new(move |dispatch_table, message_signature, superclass| {
                        dispatch_table.define_message(message_signature, move |_, args, talk_context| {
                            // Make the 'super' value part of the arguments
                            let mut args        = args;

                            if let Some(superclass) = superclass.clone() {
                                superclass.add_reference(talk_context);
                                args.push(superclass);
                            } else {
                                args.push(TalkValue::Nil);
                            }

                            // Evaluate the message
                            talk_evaluate_simple_with_arguments(Arc::clone(&parent_symbol_table), parent_frames.clone(), args.leak(), Arc::clone(&expression))
                        })
                    })
                }
            }
        );

        talk_add_class_data_reader::<SimpleEvaluatorBlockClass<TValue, TSymbol>, TalkInstanceMessageHandler>(
            |block| {
                // Clone the properties for this message handler
                let parent_symbol_table = block.parent_symbol_table.clone();
                let parent_frames       = block.parent_frames.clone();          // TODO: add/remove references to the frame cells
                let expression          = block.expression.clone();

                TalkInstanceMessageHandler {
                    define_in_dispatch_table: Box::new(move |dispatch_table, message_signature, instance_symbol_table| {
                        // Set the parent of the instance symbol table to be our existing symbol table
                        let mut instance_symbol_table   = instance_symbol_table.lock().unwrap().clone();
                        instance_symbol_table.set_parent_frame(Arc::clone(&parent_symbol_table));
                        let instance_symbol_table       = Arc::new(Mutex::new(instance_symbol_table));

                        // Bind the message
                        dispatch_table.define_message(message_signature, move |cell_reference, args, talk_context| {
                            // The data handle ID in the 'cell reference' is the 'self' cell block
                            let TalkReference(class_id, TalkDataHandle(self_cell_block)) = cell_reference.leak();
                            let self_cell_block = TalkCellBlock(self_cell_block as _);

                            // The instance cell block needs to be the first frame
                            let mut parent_frames = parent_frames.clone();
                            parent_frames.insert(0, self_cell_block);

                            // 'self' is also added as the last argument (we assume it's a cell block class here, weird things will happen if it's not)
                            talk_context.retain_cell_block(self_cell_block);
                            let mut args = args;
                            args.push(TalkValue::Reference(TalkReference(class_id, TalkDataHandle(self_cell_block.0 as _))));

                            // Evaluate the message
                            talk_evaluate_simple_with_arguments(Arc::clone(&instance_symbol_table), parent_frames, args.leak(), Arc::clone(&expression))
                                .and_then_soon(move |result, talk_context| {
                                    // The instance frame is also released at this point
                                    talk_context.release_cell_block(self_cell_block);
                                    result.into()
                                })
                        });
                    })
                }
            }
        );

        classes.insert(evaluator_type, class);
        class
    }
}

///
/// Creates a reference to a block that is evaluated using the simple evaluator
///
/// The parent_frames will be released when this block is freed, so callers need to consider that the cell blocks ownership has been transferred 
/// to the new object.
///
pub fn create_simple_evaluator_block_in_context<TValue, TSymbol>(talk_context: &mut TalkContext, arguments: Vec<TalkSymbol>, parent_frames: Vec<TalkCellBlock>, parent_symbol_table: Arc<Mutex<TalkSymbolTable>>, expression: Arc<Vec<TalkInstruction<TValue, TSymbol>>>, expression_resources: Option<TalkCellBlock>) -> TalkReference
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    // Create an argument signature
    let signature = if arguments.len() == 0 {
        TalkMessageSignature::Unary(*VALUE_SYMBOL)
    } else {
        TalkMessageSignature::Arguments(arguments.iter().map(|_| *VALUE_COLON_SYMBOL).collect())
    };

    // Create the block data
    let data        = SimpleEvaluatorBlock {
        accepted_message_id:    signature.id(),
        parent_symbol_table:    parent_symbol_table,
        parent_frames:          parent_frames,
        resources:              expression_resources,
        expression:             expression,
    };

    // Fetch the allocator for this class
    let class       = simple_evaluator_block_class::<TValue, TSymbol>();
    let allocator   = talk_context.get_callbacks_mut(class).allocator::<TalkStandardAllocator<SimpleEvaluatorBlock<TValue, TSymbol>>>().unwrap();

    // Store the data using the allocator
    let data_handle = allocator.lock().unwrap().store(data);

    TalkReference(class, data_handle)
}

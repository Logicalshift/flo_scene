use crate::flotalk::allocator::*;
use crate::flotalk::class::*;
use crate::flotalk::context::*;
use crate::flotalk::continuation::*;
use crate::flotalk::error::*;
use crate::flotalk::instruction::*;
use crate::flotalk::message::*;
use crate::flotalk::symbol::*;
use crate::flotalk::reference::*;
use crate::flotalk::releasable::*;
use crate::flotalk::simple_evaluator::*;
use crate::flotalk::symbol_table::*;
use crate::flotalk::value::*;

use smallvec::*;

use std::any::{TypeId};
use std::collections::{HashMap};
use std::marker::{PhantomData};
use std::sync::*;

lazy_static! {
    /// Maps the type IDs of the value and symbol type to the TalkClass that implements the SimpleEvaluatorClass for that ID type
    static ref SIMPLE_EVALUATOR_CLASS: Mutex<HashMap<(TypeId, TypeId), TalkClass>> = Mutex::new(HashMap::new());

    static ref VALUE_SYMBOL: TalkSymbol         = TalkSymbol::from("value");
    static ref VALUE_COLON_SYMBOL: TalkSymbol   = TalkSymbol::from("value:");
}

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
    fn release_in_context(self, context: &TalkContext) {
        // Release any cells referenced by this evaluator block
        for cell_block in self.parent_frames.iter() {
            context.release_cell_block(*cell_block);
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
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _arguments: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, _class_id: TalkClass, _allocator: &mut Self::Allocator) -> TalkContinuation<'static> {
        TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message_id)))
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, arguments: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, _reference: TalkReference, target: &mut Self::Data) -> TalkContinuation<'static> {
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
pub fn create_simple_evaluator_block_in_context<TValue, TSymbol>(talk_context: &mut TalkContext, arguments: Vec<TalkSymbol>, parent_frames: Vec<TalkCellBlock>, parent_symbol_table: Arc<Mutex<TalkSymbolTable>>, expression: Arc<Vec<TalkInstruction<TValue, TSymbol>>>) -> TalkReference
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
        expression:             expression,
    };

    // Fetch the allocator for this class
    let class       = simple_evaluator_block_class::<TValue, TSymbol>();
    let allocator   = talk_context.get_callbacks_mut(class).allocator::<TalkStandardAllocator<SimpleEvaluatorBlock<TValue, TSymbol>>>().unwrap();

    // Store the data using the allocator
    let data_handle = allocator.lock().unwrap().store(data);

    TalkReference(class, data_handle)
}

use super::allocator::*;
use super::class::*;
use super::context::*;
use super::continuation::*;
use super::error::*;
use super::instruction::*;
use super::message::*;
use super::symbol::*;
use super::reference::*;
use super::simple_evaluator::*;
use super::value::*;
use super::value_store::*;

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

    /// The symbols stored for the arguments passed into this block
    arguments:              Vec<TalkSymbol>,

    /// The captured environment for this block
    root_values:            Vec<Arc<Mutex<TalkValueStore<TalkValue>>>>,

    /// The expression to evaluate for this block
    expression:             Arc<Vec<TalkInstruction<TValue, TSymbol>>>,
}

impl<TValue, TSymbol> TalkClassDefinition for SimpleEvaluatorBlockClass<TValue, TSymbol>
where
    TValue:     'static + Send + Sync,
    TSymbol:    'static + Send + Sync,
    TalkValue:  for<'a> TryFrom<&'a TValue, Error=TalkError>,
    TalkSymbol: for<'a> From<&'a TSymbol>,
{
    type Data       = Arc<SimpleEvaluatorBlock<TValue, TSymbol>>;
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
    fn send_class_message(&self, message: TalkMessage, class_id: TalkClass, allocator: &mut Self::Allocator) -> TalkContinuation<'static> {
        TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message.signature_id())))
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message: TalkMessage, reference: TalkReference, target: &mut Self::Data) -> TalkContinuation<'static> {
        match message {
            TalkMessage::Unary(message_id) => {
                if message_id == target.accepted_message_id {
                    // Send with no arguments
                    talk_evaluate_simple(target.root_values.clone(), Arc::clone(&target.expression))
                } else {
                    // Not the message this block was expecting
                    TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message_id)))
                }
            }

            TalkMessage::WithArguments(message_id, arg_values) => {
                if message_id == target.accepted_message_id {
                    // Create a value store to store the argument values
                    let mut argument_store = TalkValueStore::default();

                    // Assume that arg_values is the same length as target.arguments
                    arg_values.into_iter()
                        .enumerate()
                        .for_each(|(idx, value)| {
                            argument_store.set_symbol_value(target.arguments[idx], value)
                        });

                    talk_evaluate_simple_with_arguments(target.root_values.clone(), argument_store, Arc::clone(&target.expression))
                } else {
                    // Not the message this block was expecting
                    TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message_id)))
                }
            }
        }
    }
}

///
/// Retrieves (or creates) the TalkClass corresponding to a simple evaluator block using the specified value and symbol types for the instructions
///
pub (super) fn simple_evaluator_block_class<TValue, TSymbol>() -> TalkClass
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
pub (super) fn simple_evaluator_block<TValue, TSymbol>(talk_context: &mut TalkContext, arguments: Vec<TalkSymbol>, root_values: Vec<Arc<Mutex<TalkValueStore<TalkValue>>>>, expression: Arc<Vec<TalkInstruction<TValue, TSymbol>>>) -> TalkReference
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
        TalkMessageSignature::Arguments(arguments.iter().map(|arg| *VALUE_COLON_SYMBOL).collect())
    };

    // Create the block data
    let data        = SimpleEvaluatorBlock {
        accepted_message_id:    signature.id(),
        arguments:              arguments,
        root_values:            root_values,
        expression:             expression,
    };

    // Fetch the allocator for this class
    let class       = simple_evaluator_block_class::<TValue, TSymbol>();
    let allocator   = talk_context.get_callbacks_mut(class).allocator::<TalkStandardAllocator<Arc<SimpleEvaluatorBlock<TValue, TSymbol>>>>().unwrap();

    // Store the data using the allocator
    let data_handle = allocator.lock().unwrap().store(Arc::new(data));

    TalkReference(class, data_handle)
}

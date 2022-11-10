use super::allocator::*;
use super::class::*;
use super::continuation::*;
use super::error::*;
use super::instruction::*;
use super::message::*;
use super::symbol::*;
use super::reference::*;
use super::simple_evaluator::*;
use super::value::*;
use super::value_store::*;

use std::sync::*;
use std::marker::{PhantomData};

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
    fn send_class_message(&self, message: TalkMessage, class_id: TalkClass, allocator: &mut Self::Allocator) -> TalkContinuation {
        TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported))
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message: TalkMessage, reference: TalkReference, target: &mut Self::Data) -> TalkContinuation {
        match message {
            TalkMessage::Unary(message_id) => {
                if message_id == target.accepted_message_id {
                    // Send with no arguments
                    talk_evaluate_simple(target.root_values.clone(), Arc::clone(&target.expression))
                } else {
                    // Not the message this block was expecting
                    TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported))
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
                    TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported))
                }
            }
        }
    }
}

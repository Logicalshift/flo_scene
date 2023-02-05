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
            todo!()
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

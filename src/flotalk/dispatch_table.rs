use super::context::*;
use super::continuation::*;
use super::error::*;
use super::message::*;
use super::sparse_array::*;
use super::value::*;

use smallvec::*;

use std::sync::*;

///
/// Maps messages to the functions that process them, and other metadata (such as the source code, or a compiled version for the intepreter)
///
#[derive(Clone)]
pub struct TalkMessageDispatchTable<TDataType> {
    /// The action to take for a particular message type
    message_action: TalkSparseArray<Arc<dyn Send + Sync + Fn(TDataType, SmallVec<[TalkValue; 4]>, &mut TalkContext) -> TalkContinuation>>,
}

impl<TDataType> TalkMessageDispatchTable<TDataType> {
    ///
    /// Creates an empty dispatch table
    ///
    pub fn empty() -> TalkMessageDispatchTable<TDataType> {
        TalkMessageDispatchTable {
            message_action: TalkSparseArray::empty()
        }
    }

    ///
    /// Builder method that can be used to initialise a dispatch table alongside its messages
    ///
    pub fn with_message<TResult>(mut self, message: impl Into<TalkMessageSignatureId>, action: impl 'static + Send + Sync + Fn(TDataType, SmallVec<[TalkValue; 4]>, &mut TalkContext) -> TResult) -> TalkMessageDispatchTable<TDataType> 
    where
        TResult: Into<TalkContinuation>,
    {
        self.define_message(message, move |data_type, args, context| action(data_type, args, context).into());

        self
    }

    ///
    /// Sends a message to an item in this dispatch table
    ///
    #[inline]
    pub fn send_message(&self, target: TDataType, message: TalkMessage, talk_context: &mut TalkContext) -> TalkContinuation {
        let id      = message.signature_id();
        let args    = message.to_arguments();

        if let Some(action) = self.message_action.get(id.into()) {
            (action)(target, args, talk_context)
        } else {
            TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(id)))
        }
    }

    ///
    /// Defines the action for a message
    ///
    pub fn define_message(&mut self, message: impl Into<TalkMessageSignatureId>, action: impl 'static + Send + Sync + Fn(TDataType, SmallVec<[TalkValue; 4]>, &mut TalkContext) -> TalkContinuation) {
        self.message_action.insert(message.into().into(), Arc::new(action));
    }
}

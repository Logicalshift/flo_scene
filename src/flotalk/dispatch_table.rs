use super::context::*;
use super::continuation::*;
use super::error::*;
use super::message::*;
use super::releasable::*;
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
    message_action: TalkSparseArray<Arc<dyn Send + Sync + for<'a> Fn(TDataType, TalkOwned<'a, SmallVec<[TalkValue; 4]>>, &'a TalkContext) -> TalkContinuation<'static>>>,
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
    pub fn with_message<TResult>(mut self, message: impl Into<TalkMessageSignatureId>, action: impl 'static + Send + Sync + for<'a> Fn(TDataType, TalkOwned<'a, SmallVec<[TalkValue; 4]>>, &'a TalkContext) -> TResult) -> Self 
    where
        TResult: Into<TalkContinuation<'static>>,
    {
        self.define_message(message, move |data_type, args, context| action(data_type, args, context).into());

        self
    }

    ///
    /// Builder method that adds all the messages from the specified table to this table
    ///
    pub fn with_messages_from(mut self, table: &TalkMessageDispatchTable<TDataType>) -> Self {
        for (message_id, message) in table.message_action.iter() {
            self.message_action.insert(message_id, Arc::clone(message));
        }

        self
    }

    ///
    /// Builder method that adds all the messages from the specified table to this table, with a map function to convert the data type
    ///
    pub fn with_mapped_messages_from<TSourceDataType>(mut self, table: &TalkMessageDispatchTable<TSourceDataType>, map_fn: impl 'static + Send + Sync + Fn(TDataType) -> TSourceDataType) -> Self 
    where
        TSourceDataType: 'static,
    {
        let map_fn = Arc::new(map_fn);

        for (message_id, message) in table.message_action.iter() {
            let map_fn  = Arc::clone(&map_fn);
            let message = Arc::clone(message);

            self.message_action.insert(message_id, Arc::new(move |data, args, context| { (message)((map_fn)(data), args, context) }));
        }

        self
    }

    ///
    /// Sends a message to an item in this dispatch table
    ///
    #[inline]
    pub fn send_message(&self, target: TDataType, message: TalkMessage, talk_context: &TalkContext) -> TalkContinuation<'static> {
        let id      = message.signature_id();
        let args    = message.to_arguments();

        if let Some(action) = self.message_action.get(id.into()) {
            (action)(target, TalkOwned::new(args, talk_context), talk_context)
        } else {
            TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(id)))
        }
    }

    ///
    /// Tries to send a message to this dispatch table, returning 'None' if no message can be sent
    ///
    #[inline]
    pub fn try_send_message(&self, target: TDataType, message: TalkMessage, talk_context: &TalkContext) -> Option<TalkContinuation<'static>> {
        let id      = message.signature_id();
        let args    = message.to_arguments();

        if let Some(action) = self.message_action.get(id.into()) {
            Some((action)(target, TalkOwned::new(args, talk_context), talk_context))
        } else {
            None
        }
    }

    ///
    /// Defines the action for a message
    ///
    pub fn define_message(&mut self, message: impl Into<TalkMessageSignatureId>, action: impl 'static + Send + Sync + for<'a> Fn(TDataType, TalkOwned<'a, SmallVec<[TalkValue; 4]>>, &'a TalkContext) -> TalkContinuation<'static>) {
        self.message_action.insert(message.into().into(), Arc::new(action));
    }
}

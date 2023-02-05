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
pub struct TalkMessageDispatchTable<TDataType>
where
    TDataType: TalkReleasable
{
    /// The action to take for a particular message type
    message_action: TalkSparseArray<Arc<dyn Send + Sync + for<'a> Fn(TalkOwned<TDataType, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static>>>,

    /// The action to take when a message is not supported
    not_supported: Arc<dyn Send + Sync + for<'a> Fn(TalkOwned<TDataType, &'a TalkContext>, TalkMessageSignatureId, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static>>,

    /// Returns true if the specified message is not in the message action list but should also be considered as supported by this dispatch table (in particular, because the not_supported function implements it)
    is_also_supported: Arc<dyn Send + Sync + Fn(TalkMessageSignatureId) -> bool>,
}

impl<TDataType> Clone for TalkMessageDispatchTable<TDataType>
where
    TDataType: TalkReleasable
{
    fn clone(&self) -> Self {
        TalkMessageDispatchTable {
            message_action:     self.message_action.clone(),
            not_supported:      self.not_supported.clone(),
            is_also_supported:  self.is_also_supported.clone(),
        }
    }
}

impl<TDataType> TalkMessageDispatchTable<TDataType>
where
    TDataType: TalkReleasable
{
    ///
    /// Creates an empty dispatch table
    ///
    pub fn empty() -> TalkMessageDispatchTable<TDataType> {
        TalkMessageDispatchTable {
            message_action:     TalkSparseArray::empty(),
            not_supported:      Arc::new(|_, id, _, _| TalkError::MessageNotSupported(id).into()),
            is_also_supported:  Arc::new(|_| false),
        }
    }

    ///
    /// Builder method that can be used to initialise a dispatch table alongside its messages
    ///
    pub fn with_message<TResult>(mut self, message: impl Into<TalkMessageSignatureId>, action: impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TDataType, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TResult) -> Self 
    where
        TResult: Into<TalkContinuation<'static>>,
    {
        self.define_message(message, move |data_type, args, context| action(data_type, args, context).into());

        self
    }

    ///
    /// Builder method that will set the action to take when an 'unsupported' message is sent to this dispatch table
    ///
    /// The default 'not supported' action is to return a MessageNotSupported error
    ///
    pub fn with_not_supported(mut self, not_supported: impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TDataType, &'a TalkContext>, TalkMessageSignatureId, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static>) -> Self {
        self.not_supported = Arc::new(not_supported);

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
        TSourceDataType: 'static + TalkReleasable,
    {
        let map_fn = Arc::new(map_fn);

        for (message_id, message) in table.message_action.iter() {
            let map_fn  = Arc::clone(&map_fn);
            let message = Arc::clone(message);

            self.message_action.insert(message_id, Arc::new(move |data, args, context| { (message)(data.map(&*map_fn), args, context) }));
        }

        self
    }

    ///
    /// Set a function to determine if a message that is not in the main message table is supported by this table
    ///
    /// This is useful to make `responds_to()` return true for messages that are not in the dispatch table but are enabled by the 'not supported' callback.
    ///
    pub fn with_is_also_supported(mut self, is_also_supported: impl 'static + Send + Sync + Fn(TalkMessageSignatureId) -> bool) -> Self {
        self.is_also_supported = Arc::new(is_also_supported);

        self
    }

    ///
    /// Sends a message to an item in this dispatch table (freeing the target when done)
    ///
    #[inline]
    pub fn send_message<'a, 'b>(&self, target: TDataType, message: TalkMessage, talk_context: &'a TalkContext) -> TalkContinuation<'b> {
        let target  = TalkOwned::new(target, talk_context);
        let id      = message.signature_id();
        let args    = TalkOwned::new(message.to_arguments(), talk_context);

        if let Some(action) = self.message_action.get(id.into()) {
            (action)(target, args, talk_context)
        } else {
            (self.not_supported)(target, id, args, talk_context)
        }
    }

    ///
    /// Tries to send a message to this dispatch table, returning 'None' if no message can be sent
    ///
    #[inline]
    pub fn try_send_message<'a>(&self, target: TDataType, message: TalkMessage, talk_context: &TalkContext) -> Option<TalkContinuation<'a>> {
        let target  = TalkOwned::new(target, talk_context);
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
    pub fn define_message(&mut self, message: impl Into<TalkMessageSignatureId>, action: impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TDataType, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static>) {
        self.message_action.insert(message.into().into(), Arc::new(action));
    }

    ///
    /// Set the action to take when an 'unsupported' message is sent to this dispatch table
    ///
    /// The default 'not supported' action is to return a MessageNotSupported error
    ///
    pub fn define_not_supported(&mut self, not_supported: impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TDataType, &'a TalkContext>, TalkMessageSignatureId, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static>) {
        self.not_supported = Arc::new(not_supported);
    }

    ///
    /// Set a function to determine if a message that is not in the main message table is supported by this table
    ///
    /// This is useful to make `responds_to()` return true for messages that are not in the dispatch table but are enabled by the 'not supported' callback.
    ///
    pub fn define_is_also_supported(&mut self, is_also_supported: impl 'static + Send + Sync + Fn(TalkMessageSignatureId) -> bool) {
        self.is_also_supported = Arc::new(is_also_supported);
    }

    ///
    /// Returns true if this dispatch table has an entry for the specified message
    ///
    #[inline]
    pub fn responds_to(&self, message_id: impl Into<TalkMessageSignatureId>) -> bool {
        let message_id = message_id.into();
        if self.message_action.get(message_id.into()).is_some() {
            true
        } else {
            (self.is_also_supported)(message_id)
        }
    }
}

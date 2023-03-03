use crate::*;
use crate::sparse_array::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::sync::*;

pub (crate) static DICTIONARY_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkDictionaryClass));

///
/// Provides the `Dictionary` FloTalk class
///
pub struct TalkDictionaryClass;

///
/// Data for the dictionary class
///
pub struct TalkDictionary {
    /// Maps hash values to key-value pairs
    buckets: TalkSparseArray<SmallVec<[(TalkValue, TalkValue); 4]>>
}

impl TalkReleasable for TalkDictionary {
    fn release_in_context(self, context: &TalkContext) {
        // Release all the values contained in the dictionary
        for (_, bucket) in self.buckets.iter() {
            for (key, value) in bucket.iter() {
                key.clone().release_in_context(context);
                value.clone().release_in_context(context);
            }
        }
    }
}

impl TalkClassDefinition for TalkDictionaryClass {
    /// The type of the data stored by an object of this class
    type Data = TalkDictionary;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkStandardAllocator<TalkDictionary>;

    ///
    /// Creates the allocator for this class in a particular context
    ///
    /// This is also an opportunity for a class to perform any other initialization it needs to do within a particular `TalkContext`
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        TalkStandardAllocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _reference: TalkReference, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Generates default dispatch table for an instance of this class
    ///
    /// Messages are dispatched here ahead of the 'send_instance_message' callback (note in particular `respondsTo:` may need to be overridden)
    ///
    fn default_instance_dispatch_table(&self) -> TalkMessageDispatchTable<TalkReference> { 
        TalkMessageDispatchTable::empty()
            .with_mapped_messages_from(&*TALK_DISPATCH_ANY, |v| TalkValue::Reference(v))

            .with_message(*TALK_MSG_ALL_SATISFY,                |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_ANY_SATISFY,                |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AS_ARRAY,                   |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AS_BAG,                     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AS_BYTE_ARRAY,              |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AS_ORDERED_COLLECTION,      |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AS_SET,                     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AS_SORTED_COLLECTION,       |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AS_SORTED_COLLECTION_COLON, |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_COLLECT,                    |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_DETECT,                     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_DETECT_IF_NONE,             |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_DO,                         |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_DO_SEPARATED_BY,            |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_INCLUDES,                   |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_INJECT_INTO,                |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_IS_EMPTY,                   |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_NOT_EMPTY,                  |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_OCCURRENCES_OF,             |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_REHASH,                     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_REJECT,                     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_SELECT,                     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_SIZE,                       |_, _, _| TalkError::NotImplemented)

            .with_message(*TALK_MSG_ADD_ALL,                    |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AT,                         |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AT_IF_ABSENT,               |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AT_PUT,                     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_INCLUDES_KEY,               |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_KEY_AT_VALUE,               |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_KEY_AT_VALUE_IF_ABSENT,     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_KEYS,                       |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_KEYS_AND_VALUES_DO,         |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_KEYS_DO,                    |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_REMOVE_ALL_KEYS,            |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_REMOVE_ALL_KEYS_IF_ABSENT,  |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_REMOVE_KEY,                 |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_REMOVE_KEY_IF_ABSENT,       |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_SELECT,                     |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_VALUES,                     |_, _, _| TalkError::NotImplemented)
    }

    ///
    /// Generates default dispatch table for the class object for this class
    ///
    /// Messages are dispatched here ahead of the 'send_instance_message' callback (note in particular `respondsTo:` may need to be overridden)
    ///
    fn default_class_dispatch_table(&self) -> TalkMessageDispatchTable<TalkClass> { 
        TalkMessageDispatchTable::empty() 
    }
}

use crate::*;
use crate::sparse_array::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::mem;
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

impl TalkDictionary {
    ///
    /// Adds a new value to this dictionary
    ///
    pub (crate) fn add_value(dictionary: TalkOwned<TalkReference, &'_ TalkContext>, key: TalkOwned<TalkValue, &'_ TalkContext>, value: TalkOwned<TalkValue, &'_ TalkContext>, context: &TalkContext) -> TalkContinuation<'static> {
        // Fetch the allocator for the dictionary class
        let dictionary  = dictionary.leak();
        let allocator   = dictionary.class().allocator_ref::<TalkStandardAllocator<TalkDictionary>>(context);
        let allocator   = if let Some(allocator) = allocator { allocator } else { return TalkError::UnexpectedClass.into(); };

        let key         = key.leak();
        let value       = value.leak();

        // Fetch the hash value from the key to start with
        key.clone_in_context(context)
            .send_message_in_context(TalkMessage::Unary(*TALK_MSG_HASH), context)
            .and_then_soon(move |hash_value, context| {
                // If there's an error, free up the various values
                if let Ok(err) = hash_value.try_as_error() {
                    vec![TalkValue::from(dictionary), key, value, hash_value].release_in_context(context);
                    return err.into();
                }

                // Look up the hash in the buckets
                let hash_value = if let TalkValue::Int(hash_value) = hash_value {
                    hash_value
                } else { 
                    // Hash value is not an integer: free up the various values
                    vec![TalkValue::from(dictionary), key, value, hash_value].release_in_context(context);
                    return TalkError::NotAnInteger.into();
                };

                // Store the value at the end of the buckets
                let hash_value          = hash_value as usize;
                let mut allocator_lock  = allocator.lock().unwrap();
                let dictionary_data     = allocator_lock.retrieve(dictionary.data_handle());

                let bucket = if let Some(bucket) = dictionary_data.buckets.get_mut(hash_value) {
                    bucket
                } else {
                    dictionary_data.buckets.insert(hash_value, smallvec![]);
                    dictionary_data.buckets.get_mut(hash_value).unwrap()
                };

                bucket.push((key.clone(), value));

                // (Success) Remove anything that has a duplicate of the key
                let mut found_key = false;
                for idx in 0..(bucket.len()-1) {
                    if bucket[idx].0 == key {
                        bucket.remove(idx);
                        found_key = true;
                    }
                }

                if !found_key && bucket.len() > 1 {
                    // The bucket has more than one item and we didn't find the key we've just added: we need to call '=' on the 
                    // remaining values in order to determine if we should remove them. This reverses the values so removal operations
                    // don't interfere with each other
                    let continuations = bucket.iter().enumerate()
                        .take(bucket.len()-1)
                        .rev()
                        .map(|(idx, (item_key, _))| {
                            let allocator   = Arc::clone(&allocator);
                            let dictionary  = dictionary.clone_in_context(context);
                            key.clone_in_context(context).send_message(TalkMessage::WithArguments(*TALK_BINARY_EQUALS, smallvec![item_key.clone_in_context(context)]))
                                .and_then_soon(move |is_equal, context| {
                                    if is_equal == TalkValue::Bool(true) {
                                        let mut allocator_lock  = allocator.lock().unwrap();
                                        let dictionary_data     = allocator_lock.retrieve(dictionary.data_handle());

                                        dictionary_data.buckets.get_mut(hash_value).unwrap().remove(idx);
                                    }

                                    dictionary.release_in_context(context);
                                    ().into()
                                })
                        }).collect::<SmallVec<[_; 4]>>();

                    // Chain the continuations together into a single continuation
                    let mut result = TalkContinuation::from(());
                    for continuation in continuations {
                        result = result.and_then_soon(move |_, _| continuation);
                    }

                    result
                } else {
                    // The bucket only has the one entry or we found a copy of the key already in the bucket: result is successful
                    mem::drop(allocator_lock);

                    dictionary.release_in_context(context);
                    ().into()
                }
            })
    }

    ///
    /// Looks up a key in the dictionary, then performs one of two actions depending on whether or not it exists 
    ///
    pub (crate) fn process_value_for_key(dictionary: TalkOwned<TalkReference, &'_ TalkContext>, key: TalkOwned<TalkValue, &'_ TalkContext>, 
        if_exists: impl 'static + Send + FnOnce(TalkOwned<TalkValue, &'_ TalkContext>, &TalkContext) -> TalkContinuation<'static>, 
        if_doesnt_exist: impl 'static + Send + FnOnce(&mut TalkContext) -> TalkContinuation<'static>,
        context: &TalkContext) -> TalkContinuation<'static> {
        // Grab the dictionary to pass on to the following continuation
        let dictionary  = dictionary.leak();
        let key         = key.leak();

        // Also need the allocator to get at the dictionary data
        let allocator   = dictionary.class().allocator_ref::<TalkStandardAllocator<TalkDictionary>>(context);
        let allocator   = if let Some(allocator) = allocator { allocator } else { return TalkError::UnexpectedClass.into(); };

        // Start by requesting the hash value from the key (we need to keep it around for comparisons later too)
        key.clone_in_context(context)
            .send_message_in_context(TalkMessage::Unary(*TALK_MSG_HASH), context)
            .and_then_soon(move |hash_value, context| {
                // Stop if there's an error
                if let Ok(err) = hash_value.try_as_error() {
                    vec![key, dictionary.into(), hash_value].release_in_context(context);
                    return err.into();
                }

                // Also stop if there's no hash value
                let hash_value = if let TalkValue::Int(hash_value) = hash_value {
                    hash_value as usize
                } else {
                    // Not a hash value
                    vec![key, dictionary.into(), hash_value].release_in_context(context);
                    return TalkError::NotAnInteger.into();
                };

                // Look up the bucket and search it for the key
                let mut allocator_lock  = allocator.lock().unwrap();
                let dictionary_data     = allocator_lock.retrieve(dictionary.data_handle());
                let bucket              = dictionary_data.buckets.get(hash_value);

                let bucket = if let Some(bucket) = bucket {
                    bucket
                } else {
                    // Bucket not found, so the key is not in the dictionary
                    mem::drop(allocator_lock);

                    vec![key, dictionary.into()].release_in_context(context);
                    return if_doesnt_exist(context);
                };

                // Check the bucket for a reference duplicate of the key, and return 'if_exists' if found
                for (bucket_key, bucket_value) in bucket.iter() {
                    if &key == bucket_key {
                        // Key is the same reference as the value in the dictionary, so we can avoid using the '=' message
                        let context         = &*context;
                        let bucket_value    = TalkOwned::new(bucket_value.clone_in_context(context), context);

                        mem::drop(allocator_lock);

                        vec![key, dictionary.into()].release_in_context(context);
                        return if_exists(bucket_value, context);
                    }
                }

                // If an exact reference match of the key is not found, try harder by checking for equality with the '=' message
                let bucket = bucket.iter()
                    .map(|(key, value)| (key.clone_in_context(context), value.clone_in_context(context)))
                    .collect::<SmallVec<[_; 4]>>();

                let bucket = bucket.into_iter();

                // next_bucket is a function used to repeatedly generate continuations while there are still items to compare in the bucket
                fn next_bucket(key: TalkValue, mut bucket: impl 'static + Send + Iterator<Item=(TalkValue, TalkValue)>,
                    if_exists: impl 'static + Send + FnOnce(TalkOwned<TalkValue, &'_ TalkContext>, &TalkContext) -> TalkContinuation<'static>, 
                    if_doesnt_exist: impl 'static + Send + FnOnce(&mut TalkContext) -> TalkContinuation<'static>,
                    context: &mut TalkContext) -> TalkContinuation<'static> {
                    if let Some((bucket_key, bucket_value)) = bucket.next() {
                        // Check the next key and then decide what to do
                        key.clone_in_context(context).send_message_in_context(TalkMessage::WithArguments(*TALK_BINARY_EQUALS, smallvec![bucket_key]), context)
                            .and_then_soon(move |is_equal, context| {
                                if is_equal == TalkValue::Bool(true) {
                                    // Found the value: release the remaining other values
                                    for (unused_key, unused_value) in bucket {
                                        unused_key.release_in_context(context);
                                        unused_value.release_in_context(context);
                                    }

                                    key.release_in_context(context);

                                    // Pass the value we found on to if_exists
                                    if_exists(TalkOwned::new(bucket_value, context), context)
                                } else {
                                    // Continue to the next value
                                    is_equal.release_in_context(context);
                                    next_bucket(key, bucket, if_exists, if_doesnt_exist, context)
                                }
                            })
                    } else {
                        // Reached the end of the iterator without finding the key
                        key.release_in_context(context);
                        if_doesnt_exist(context)
                    }
                }

                mem::drop(allocator_lock);

                dictionary.release_in_context(context);
                next_bucket(key, bucket, if_exists, if_doesnt_exist, context)
            })
    }

    #[inline]
    fn new(context: &TalkContext) -> TalkContinuation<'static> {
        // Fetch the allocator from the context
        let allocator       = DICTIONARY_CLASS.allocator_ref::<TalkStandardAllocator<TalkDictionary>>(context);
        let allocator       = if let Some(allocator) = allocator { allocator } else { return TalkError::UnexpectedClass.into(); };
        let mut allocator   = allocator.lock().unwrap();

        // Create a new dictionary object
        let new_dictionary = TalkDictionary { 
            buckets: TalkSparseArray::empty()
        };

        // Store in the allocator
        let new_dictionary = allocator.store(new_dictionary);

        // Result is a reference to the new dictionary object
        TalkReference(*DICTIONARY_CLASS, new_dictionary).into()
    }

    #[inline]
    fn at(dictionary: TalkOwned<TalkReference, &'_ TalkContext>, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, context: &TalkContext) -> TalkContinuation<'static> {
        let mut args    = args.leak();
        let key         = TalkOwned::new(args[0].take(), context);

        Self::process_value_for_key(dictionary, key, |value, _| value.leak().into(), |_| ().into(), context)
    }

    #[inline]
    fn at_put(dictionary: TalkOwned<TalkReference, &'_ TalkContext>, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, context: &TalkContext) -> TalkContinuation<'static> {
        let mut args    = args.leak();
        let key         = TalkOwned::new(args[0].take(), context);
        let value       = TalkOwned::new(args[1].take(), context);

        Self::add_value(dictionary, key, value, context)
    }
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
            .with_message(*TALK_MSG_AT,                         |dict, args, context| TalkDictionary::at(dict, args, context))
            .with_message(*TALK_MSG_AT_IF_ABSENT,               |_, _, _| TalkError::NotImplemented)
            .with_message(*TALK_MSG_AT_PUT,                     |dict, args, context| TalkDictionary::at_put(dict, args, context))
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
            .with_message(*TALK_MSG_NEW, |_, _, context| TalkDictionary::new(context))
    }
}

use crate::allocator::*;
use crate::continuation::*;
use crate::class::*;
use crate::context::*;
use crate::error::*;
use crate::message::*;
use crate::releasable::*;
use crate::reference::*;
use crate::value::*;
use crate::value_messages::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::any::{TypeId};
use std::collections::{HashMap};
use std::marker::{PhantomData};
use std::sync::*;

static FN_BLOCK_CLASS: Lazy<Mutex<HashMap<TypeId, TalkClass>>> = Lazy::new(|| Mutex::new(HashMap::new()));

///
/// Represents a function block class (a call to a particular rust function, invoked by calling 'value:' on any instance)
///
pub struct TalkFnBlock<TFn, TParamType> 
where
    TFn:            'static + Send + Fn(TParamType) -> TalkContinuation<'static>,
    TParamType:     'static + Send + TalkValueType,
{
    callback:       PhantomData<Arc<Mutex<TFn>>>,
    param:          PhantomData<Arc<Mutex<TParamType>>>,
}

pub struct TalkFnData<TFn, TParamType>
where
    TFn:            'static + Send + Fn(TParamType) -> TalkContinuation<'static>,
    TParamType:     'static + Send + TalkValueType,
{
    func:           Arc<Mutex<TFn>>,
    param:          PhantomData<Arc<Mutex<TParamType>>>,
}

impl<TFn, TParamType> TalkReleasable for TalkFnData<TFn, TParamType> 
where
    TFn:            'static + Send + Fn(TParamType) -> TalkContinuation<'static>,
    TParamType:     'static + Send + TalkValueType,
{
    #[inline]
    fn release_in_context(self, _context: &TalkContext) { }
}

impl<TFn, TParamType> TalkClassDefinition for TalkFnBlock<TFn, TParamType>
where
    TFn:            'static + Send + Fn(TParamType) -> TalkContinuation<'static>,
    TParamType:     'static + Send + TalkValueType,
{
    /// The type of the data stored by an object of this class
    type Data = TalkFnData<TFn, TParamType>;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkStandardAllocator<Self::Data>;

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
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        if message_id == *TALK_MSG_VALUE_COLON {
            // Data handle points to the function data in the allocator
            let data_handle = reference.data_handle();

            // Retrieve the function reference
            let callback = allocator.lock().unwrap().retrieve(data_handle).func.clone();

            // Convert the argument to the parameter type
            let mut args = args;
            let argument = TalkOwned::new(args[0].take(), args.context());

            match TParamType::try_from_talk_value(argument, args.context()) {
                Err(err)        => err.into(),

                Ok(parameter)   =>  {
                    // Invoke the function in a continuation (so any locks used by send_instance_message are released)
                    (callback.lock().unwrap())(parameter)
                }
            }
        } else {
            TalkError::MessageNotSupported(message_id).into()
        }
    }
}

impl<TFn, TParamType> TalkFnBlock<TFn, TParamType>
where
    TFn:            'static + Send + Fn(TParamType) -> TalkContinuation<'static>,
    TParamType:     'static + Send + TalkValueType,
{
    ///
    /// Returns the class ID for this function block type
    ///
    pub fn class() -> TalkClass {
        let our_type    = TypeId::of::<Self>();
        let mut classes = FN_BLOCK_CLASS.lock().unwrap();

        if let Some(class_id) = classes.get(&our_type) {
            *class_id
        } else {
            let class_id = TalkClass::create(TalkFnBlock { callback: PhantomData::<Arc<Mutex<TFn>>>, param: PhantomData });
            classes.insert(our_type, class_id);

            class_id
        }
    }
}

///
/// Creates a function block in a context (supports the 'value:' message to call the rust function)
///
pub fn talk_fn_block_continuation_in_context<'a, TFn, TParamType>(callback: TFn, context: &'a mut TalkContext) -> TalkOwned<TalkReference, &'a TalkContext> 
where
    TFn:            'static + Send + Fn(TParamType) -> TalkContinuation<'static>,
    TParamType:     'static + Send + TalkValueType,
{
    // Fetch the allocator for this class ID
    let class_id    = TalkFnBlock::<TFn, TParamType>::class();
    let callbacks   = context.get_callbacks_mut(class_id);
    let allocator   = callbacks.allocator.downcast_ref::<Arc<Mutex<<TalkFnBlock::<TFn, TParamType> as TalkClassDefinition>::Allocator>>>()
            .map(|defn| Arc::clone(defn))
            .unwrap();

    // Create a data object with the function definition in it
    let data        = TalkFnData {
        func:           Arc::new(Mutex::new(callback)),
        param:          PhantomData,
    };

    let data_handle = allocator.lock().unwrap().store(data);

    // Reference is made up of the class and data handle
    let reference = TalkReference(class_id, data_handle);
    TalkOwned::new(reference, context)
}

///
/// Creates a function block in a context (supports the 'value:' message to call the rust function)
///
pub fn talk_fn_block_in_context<'a, TFn, TParamType, TReturnValue>(callback: TFn, context: &'a mut TalkContext) -> TalkOwned<TalkReference, &'a TalkContext> 
where
    TFn:            'static + Send + Fn(TParamType) -> TReturnValue,
    TParamType:     'static + Send + TalkValueType,
    TReturnValue:   'static + Send + TalkValueType,
{
    // Make the callback return a continuation
    let callback    = move |param| {
        let result = callback(param);
        TalkContinuation::soon(move |talk_context| {
            result.into_talk_value(talk_context).leak().into()
        })
    };

    // Create a continuation fn_block
    talk_fn_block_continuation_in_context(callback, context)
}

///
/// Creates a function block using a continuation (the returned reference supports the 'value:' message to call the rust function)
///
/// A function block can be used in a case where a FloTalk routine needs to call back into a rust routine: it acts like a FloTalk block
/// that is declared with a single parameter. For example, the `do:` iterator callback can be made to call into a Rust routine using
/// something like:
///
/// ```
/// # use flo_talk::*;
/// # let target_reference = TalkValue::Nil;
/// let call_do = talk_fn_block(|x: i32| { println!("{}", x); })
///     .and_then_soon_if_ok(move |do_block, talk_context| {
///         target_reference.send_message_in_context(TalkMessage::with_arguments(vec![("do:", do_block)]), talk_context)
///     });
/// ```
///
/// Note that using a stream is generally a much more straightforward way to interface between Rust and FloTalk: this is mostly useful
/// when a stream type is not available for some reason (the 'do:' function on a collection in this case).
///
pub fn talk_fn_block<'a, TFn, TParamType, TReturnValue>(callback: TFn) -> TalkContinuation<'static> 
where
    TFn:            'static + Send + Fn(TParamType) -> TReturnValue,
    TParamType:     'static + Send + TalkValueType,
    TReturnValue:   'static + Send + TalkValueType,
{
    TalkContinuation::soon(move |talk_context| talk_fn_block_in_context(callback, talk_context).leak().into())
}

///
/// Creates a function block that returns a continuation to generate its result
///
pub fn talk_fn_block_continuation<'a, TFn, TParamType>(callback: TFn) -> TalkContinuation<'static> 
where
    TFn:            'static + Send + Fn(TParamType) -> TalkContinuation<'static>,
    TParamType:     'static + Send + TalkValueType,
{
    TalkContinuation::soon(move |talk_context| talk_fn_block_continuation_in_context(callback, talk_context).leak().into())
}

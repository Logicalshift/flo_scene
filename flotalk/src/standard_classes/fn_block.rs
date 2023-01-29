use crate::allocator::*;
use crate::continuation::*;
use crate::class::*;
use crate::context::*;
use crate::error::*;
use crate::message::*;
use crate::releasable::*;
use crate::reference::*;
use crate::value::*;

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
pub struct TalkFnBlock<TFn, TParamType, TReturnValue> 
where
    TFn:            'static + Send + Fn(TParamType) -> TReturnValue,
    TParamType:     'static + Send + TalkValueType,
    TReturnValue:   'static + Send + TalkValueType,
{
    callback:       PhantomData<Arc<Mutex<TFn>>>,
    param:          PhantomData<Arc<Mutex<TParamType>>>,
    return_value:   PhantomData<Arc<Mutex<TReturnValue>>>,
}

pub struct TalkFnData<TFn, TParamType, TReturnValue>
where
    TFn:            'static + Send + Fn(TParamType) -> TReturnValue,
    TParamType:     'static + Send + TalkValueType,
    TReturnValue:   'static + Send + TalkValueType,
{
    func:           Arc<Mutex<TFn>>,
    param:          PhantomData<Arc<Mutex<TParamType>>>,
    return_value:   PhantomData<Arc<Mutex<TReturnValue>>>,
}

impl<TFn, TParamType, TReturnValue> TalkReleasable for TalkFnData<TFn, TParamType, TReturnValue> 
where
    TFn:            'static + Send + Fn(TParamType) -> TReturnValue,
    TParamType:     'static + Send + TalkValueType,
    TReturnValue:   'static + Send + TalkValueType,
{
    #[inline]
    fn release_in_context(self, _context: &TalkContext) { }
}

impl<TFn, TParamType, TReturnValue> TalkClassDefinition for TalkFnBlock<TFn, TParamType, TReturnValue>
where
    TFn:            'static + Send + Fn(TParamType) -> TReturnValue,
    TParamType:     'static + Send + TalkValueType,
    TReturnValue:   'static + Send + TalkValueType,
{
    /// The type of the data stored by an object of this class
    type Data = TalkFnData<TFn, TParamType, TReturnValue>;

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
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _reference: TalkReference, _allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}

impl<TFn, TParamType, TReturnValue> TalkFnBlock<TFn, TParamType, TReturnValue>
where
    TFn:            'static + Send + Fn(TParamType) -> TReturnValue,
    TParamType:     'static + Send + TalkValueType,
    TReturnValue:   'static + Send + TalkValueType,
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
            let class_id = TalkClass::create(TalkFnBlock { callback: PhantomData::<Arc<Mutex<TFn>>>, param: PhantomData, return_value: PhantomData });
            classes.insert(our_type, class_id);

            class_id
        }
    }
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
    // Fetch the allocator for this class ID
    let class_id    = TalkFnBlock::<TFn, TParamType, TReturnValue>::class();
    let callbacks   = context.get_callbacks_mut(class_id);
    let allocator   = callbacks.allocator.downcast_ref::<Arc<Mutex<<TalkFnBlock::<TFn, TParamType, TReturnValue> as TalkClassDefinition>::Allocator>>>()
            .map(|defn| Arc::clone(defn))
            .unwrap();

    // Create a data object with the function definition in it
    let data        = TalkFnData {
        func:           Arc::new(Mutex::new(callback)),
        param:          PhantomData,
        return_value:   PhantomData
    };

    let data_handle = allocator.lock().unwrap().store(data);

    // Reference is made up of the class and data handle
    let reference = TalkReference(class_id, data_handle);
    TalkOwned::new(reference, context)
}

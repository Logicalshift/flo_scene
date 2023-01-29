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

use std::marker::{PhantomData};
use std::sync::*;

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

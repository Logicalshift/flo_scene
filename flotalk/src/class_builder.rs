use super::allocator::*;
use super::continuation::*;
use super::message::*;
use super::releasable::*;

use std::collections::{HashMap};

///
/// The class builder can be used to build a FloTalk class quickly from Rust
///
pub struct TalkClassBuilder<TDataType> 
where
    TDataType: TalkReleasable,
{
    /// The name of the class that is being built
    name: String,

    /// Supported instance messages
    instance_messages:  HashMap<TalkMessageSignatureId, Box<dyn Send + Sync + for<'a> Fn(TalkMessage, &'a mut TDataType) -> TalkContinuation<'static>>>,

    /// Supported class messages
    class_messages:     HashMap<TalkMessageSignatureId, Box<dyn Send + Sync + for<'a> Fn(&'a mut TalkStandardAllocator<TDataType>, TalkMessage) -> TalkContinuation<'static>>>,
}

///
/// Trait implemented by things that can be converted into a class instance function
///
pub trait TalkIntoInstanceFn<TDataType> {
    /// The number of arguments accepted for the message for this instance function
    fn num_arguments(&self) -> usize;

    /// Creates an instance function for this function
    fn into_instance_fn(self) -> Box<dyn Send + Sync + for<'a> Fn(TalkMessage, &'a mut TDataType) -> TalkContinuation<'static>>;
}

///
/// Trait implemented by things that can be converted into a class instance function
///
pub trait TalkIntoClassFn<TDataType>
where
    TDataType: TalkReleasable,
{
    /// The number of arguments accepted for the message for this instance function
    fn num_arguments(&self) -> usize;

    /// Creates an instance function for this function
    fn into_class_fn(self) -> Box<dyn Send + Sync + for<'a> Fn(&'a mut TalkStandardAllocator<TDataType>, TalkMessage) -> TalkContinuation<'static>>;
}

impl<TDataType> TalkClassBuilder<TDataType>
where
    TDataType: TalkReleasable,
{
    ///
    /// Begins building a class
    ///
    pub fn new_class(name: &str) -> TalkClassBuilder<TDataType> {
        TalkClassBuilder {
            name:               name.to_string(),
            instance_messages:  HashMap::new(),
            class_messages:     HashMap::new(),
        }
    }

    ///
    /// Adds an instance method to this class definition
    ///
    pub fn with_method(&mut self, signature: impl Into<TalkMessageSignatureId>, new_method: impl TalkIntoInstanceFn<TDataType>) -> &mut Self {
        self.instance_messages.insert(signature.into(), new_method.into_instance_fn());

        self
    }
}

impl<TDataType, TContinuation, TFn> TalkIntoInstanceFn<TDataType> for TFn
where
    TFn:            'static + Send + Sync + Fn(&mut TDataType) -> TContinuation,
    TContinuation:  Into<TalkContinuation<'static>>,
{
    fn num_arguments(&self) -> usize {
        0
    }

    fn into_instance_fn(self) -> Box<dyn Send + Sync + for<'a> Fn(TalkMessage, &'a mut TDataType) -> TalkContinuation<'static>> {
        Box::new(move |_msg, data| {
            (self)(data).into()
        })
    }
}

/* -- TODO, conflicts with above but we want to extend the implementation this way
impl<TDataType, TContinuation, TFn> TalkIntoInstanceFn<TDataType> for TFn
where
    TFn:            'static + Send + Sync + Fn(&mut TDataType, TalkValue) -> TContinuation,
    TContinuation:  Into<TalkContinuation>,
{
    fn num_arguments(&self) -> usize {
        1
    }

    fn into_instance_fn(self) -> Box<dyn Send + Sync + for<'a> Fn(TalkMessage, &'a mut TDataType) -> TalkContinuation<'static>> {
        Box::new(move |msg, data| {
            let arg = match msg {
                TalkMessage::Unary(_)               => TalkValue::Nil,
                TalkMessage::WithArguments(_, args) => args[0],
            };

            (self)(data, arg).into()
        })
    }
}
*/

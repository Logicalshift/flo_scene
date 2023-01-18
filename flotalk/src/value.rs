use super::context::*;
use super::continuation::*;
use super::number::*;
use super::error::*;
use super::expression::*;
use super::message::*;
use super::reference::*;
use super::releasable::*;
use super::symbol::*;

use smallvec::*;

use std::f64;
use std::i64;
use std::u32;
use std::str::{FromStr};
use std::sync::*;
use std::mem;
use std::hash;

///
/// Trait implemented by types that can be converted into a TalkValue
///
/// It's valid to implement `From<T>` alongside this trait, or this trait by itself: this trait is useful for the case where
/// the conversion requires the use of the context or can generate an error.
///
pub trait TalkValueType : Sized {
    ///
    /// Tries to convert this item into a TalkValue
    ///
    fn into_talk_value<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkValue, &'a TalkContext>;

    ///
    /// Tries to convert a TalkValue into this item
    ///
    fn try_from_talk_value<'a>(value: TalkOwned<TalkValue, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError>;
}

///
/// The result of a FloTalk message
///
/// Note that cloning a value does not increase the reference count for any data that's referenced. Use `clone_in_context()` for that.
///
#[derive(Clone, PartialEq, Debug)]
pub enum TalkValue {
    /// The 'nil' value
    Nil,

    /// A reference to a value
    Reference(TalkReference),

    /// A boolean value
    Bool(bool),

    /// An integer value
    Int(i64),

    /// A floating point value
    Float(f64),

    /// A string value
    String(Arc<String>),

    /// A character value
    Character(char),

    /// A symbol value
    Symbol(TalkSymbol),

    /// A symbol representing a selector
    Selector(TalkMessageSignatureId),

    /// An array of values
    Array(Vec<TalkValue>),

    /// A message
    Message(Box<TalkMessage>),

    /// An error
    Error(TalkError),
}

impl Default for TalkValue {
    fn default() -> TalkValue {
        TalkValue::Nil
    }
}

impl TalkValue {
    ///
    /// Moves this value, replacing it with the value 'Nil'
    ///
    /// This is useful for retrieving values from `TalkOwned`, but note that they have to be manually released after this
    ///
    #[inline]
    pub fn take(&mut self) -> TalkValue {
        let mut value = TalkValue::Nil;
        mem::swap(self, &mut value);
        value
    }

    ///
    /// Returns the reference represented by this value
    ///
    pub fn try_as_reference(&self) -> Result<&TalkReference, TalkError> {
        match self {
            TalkValue::Nil                  => Err(TalkError::IsNil),
            TalkValue::Reference(value_ref) => Ok(value_ref),
            _                               => Err(TalkError::NotAReference)
        }
    }

    ///
    /// Returns the reference represented by this value
    ///
    pub fn try_as_bool(&self) -> Result<bool, TalkError> {
        match self {
            TalkValue::Nil                  => Err(TalkError::IsNil),
            TalkValue::Bool(val)            => Ok(*val),
            _                               => Err(TalkError::NotABoolean)
        }
    }

    ///
    /// Returns the reference represented by this value
    ///
    pub fn try_as_int(&self) -> Result<i64, TalkError> {
        match self {
            TalkValue::Nil                  => Err(TalkError::IsNil),
            TalkValue::Int(num)             => Ok(*num),
            TalkValue::Float(num)           => Ok(*num as i64),
            _                               => Err(TalkError::NotAnInteger)
        }
    }

    ///
    /// Returns the reference represented by this value
    ///
    pub fn try_as_float(&self) -> Result<f64, TalkError> {
        match self {
            TalkValue::Nil                  => Err(TalkError::IsNil),
            TalkValue::Int(num)             => Ok(*num as f64),
            TalkValue::Float(num)           => Ok(*num),
            _                               => Err(TalkError::NotAFloat)
        }
    }

    ///
    /// Returns the reference represented by this value
    ///
    pub fn try_as_number(&self) -> Result<TalkNumber, TalkError> {
        match self {
            TalkValue::Nil                  => Err(TalkError::IsNil),
            TalkValue::Int(num)             => Ok(TalkNumber::Int(*num)),
            TalkValue::Float(num)           => Ok(TalkNumber::Float(*num)),
            _                               => Err(TalkError::NotAFloat)
        }
    }

    ///
    /// Returns true if this reference is to a `TalkClass` object
    ///
    pub fn is_class_object(&self) -> bool {
        match self {
            TalkValue::Reference(reference) => reference.is_class_object(),
            _                               => false,
        }
    }

    ///
    /// Sends a message to this value, then releases it
    ///
    #[inline]
    pub fn send_message_in_context<'a>(self, message: TalkMessage, context: &TalkContext) -> TalkContinuation<'a> {
        use TalkValue::*;

        match self {
            Nil                         => context.value_dispatch_tables.any_dispatch.send_message(TalkValue::Nil, message, context),
            Bool(bool_value)            => context.value_dispatch_tables.bool_dispatch.send_message(bool_value, message, context),
            Int(int_value)              => context.value_dispatch_tables.int_dispatch.send_message(TalkNumber::Int(int_value), message, context),
            Float(float_value)          => context.value_dispatch_tables.float_dispatch.send_message(TalkNumber::Float(float_value), message, context),
            String(string_value)        => context.value_dispatch_tables.string_dispatch.send_message(string_value, message, context),
            Character(char_value)       => context.value_dispatch_tables.character_dispatch.send_message(char_value, message, context),
            Symbol(symbol_value)        => context.value_dispatch_tables.symbol_dispatch.send_message(symbol_value, message, context),
            Selector(selector_value)    => context.value_dispatch_tables.selector_dispatch.send_message(selector_value, message, context),
            Array(array_value)          => context.value_dispatch_tables.array_dispatch.send_message(array_value, message, context),
            Error(error)                => context.value_dispatch_tables.error_dispatch.send_message(error, message, context),
            Message(msg)                => context.value_dispatch_tables.message_dispatch.send_message(msg, message, context),

            Reference(reference)        => reference.send_message_in_context(message, context),
        }
    }

    ///
    /// Sends a message to this value, then releases it
    ///
    /// This differs from `send_message` in that `send_message` assumes that the caller is always using the correct number of arguments for the
    /// selector. This will check, and only send the message if the arguments match.
    ///
    #[inline]
    pub (super) fn perform_message_in_context<'a>(self, message_id: TalkMessageSignatureId, arguments: TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, context: &'a TalkContext) -> TalkContinuation<'static> {
        if message_id.len() != arguments.len() {
            // Selector does not match the arguments
            TalkError::WrongNumberOfArguments.into()
        } else {
            // Send the message
            let message = if message_id.len() == 0 { TalkMessage::Unary(message_id) } else { TalkMessage::WithArguments(message_id, arguments.leak()) };
            self.send_message_in_context(message, context)
        }
    }

    ///
    /// Sends a message to this value, then releases it
    ///
    #[inline]
    pub fn send_message<'a>(self, message: TalkMessage) -> TalkContinuation<'a> {
        TalkContinuation::soon(move |talk_context| self.send_message_in_context(message, talk_context))
    }

    ///
    /// Increases the reference count for this value. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn retain(&self, context: &TalkContext) {
        use TalkValue::*;

        match self {
            Nil             |
            Bool(_)         |
            Int(_)          |
            Float(_)        |
            String(_)       |
            Character(_)    |
            Symbol(_)       |
            Selector(_)     |
            Error(_)        => { }

            Reference(reference)    => reference.retain(context),
            Array(values)           => values.iter().for_each(|val| val.retain(context)),
            Message(msg)            => msg.retain(context),
        }
    }

    ///
    /// Decreases the reference count for this value. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn release(&self, context: &TalkContext) {
        use TalkValue::*;

        match self {
            Nil             |
            Bool(_)         |
            Int(_)          |
            Float(_)        |
            String(_)       |
            Character(_)    |
            Symbol(_)       |
            Selector(_)     |
            Error(_)        => { }

            Reference(reference)    => reference.release(context),
            Array(values)           => values.iter().for_each(|val| val.release(context)),
            Message(msg)            => msg.release(context),
        }
    }

    ///
    /// Parses a radix number (eg: 16rF00D)
    ///
    fn parse_radix_number(number: &str) -> Result<TalkValue, TalkError> {
        // Split the value around the 'r'
        let components = number.split("r").collect::<SmallVec<[_; 2]>>();

        if components.len() != 2 {
            return Err(TalkError::InvalidRadixNumber(number.to_string()));
        }

        // Parse the radix
        let radix = u32::from_str(&components[0])
            .map_err(|_| TalkError::InvalidRadixNumber(number.to_string()))?;

        // Parse the number
        let number = i64::from_str_radix(&components[1], radix)
            .map_err(|_| TalkError::InvalidRadixNumber(number.to_string()))?;

        Ok(TalkValue::Int(number))
    }

    ///
    /// Attempts to parse a talk value as a number
    ///
    pub fn parse_number(number: &str) -> Result<TalkValue, TalkError> {
        if number.contains('r') || number.contains('R') {
            // Radix number
            Self::parse_radix_number(number)
        } else if number.contains('e') || number.contains('E') || number.contains('.') {
            // Floating point number
            f64::from_str(number)
                .map(|num| TalkValue::Float(num))
                .map_err(|_| TalkError::InvalidFloatingPointNumber(number.to_string()))
        } else {
            // Integer
            i64::from_str(number)
                .map(|num| TalkValue::Int(num))
                .map_err(|_| TalkError::InvalidIntegerNumber(number.to_string()))
        }
    }

    ///
    /// Return the data for a reference cast to a target type (if it can be read as that type)
    ///
    pub fn read_data_in_context<TTargetData>(&self, context: &TalkContext) -> Option<TTargetData> 
    where
        TTargetData: 'static,
    {
        match self {
            TalkValue::Reference(reference) => reference.read_data_in_context(context),
            _                               => None,
        }
    }

 }

impl hash::Hash for TalkValue {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher 
    {
        use TalkValue::*;
        match self {
            Nil                         => { 0.hash(state); },
            Bool(bool_value)            => { 1.hash(state); bool_value.hash(state); }
            Int(int_value)              => { 2.hash(state); int_value.hash(state); }
            Float(float_value)          => { let bits = float_value.to_bits(); 3.hash(state); bits.hash(state); }
            String(string_value)        => { 4.hash(state); string_value.hash(state); }
            Character(char_value)       => { 5.hash(state); char_value.hash(state); }
            Symbol(symbol_value)        => { 6.hash(state); symbol_value.hash(state); }
            Selector(selector_value)    => { 7.hash(state); selector_value.hash(state); }
            Array(array_value)          => { 8.hash(state); array_value.hash(state); }
            Error(error)                => { 9.hash(state); error.hash(state); }
            Reference(reference)        => { 10.hash(state); reference.hash(state); }
            Message(message_value)      => { 11.hash(state); message_value.hash(state); }
        }
    }
}

impl From<()> for TalkValue {
    fn from(_val: ()) -> TalkValue { TalkValue::Nil }
}

impl From<TalkReference> for TalkValue {
    fn from(val: TalkReference) -> TalkValue { TalkValue::Reference(val) }
}

impl From<bool> for TalkValue {
    fn from(val: bool) -> TalkValue { TalkValue::Bool(val) }
}

impl From<i32> for TalkValue {
    fn from(val: i32) -> TalkValue { TalkValue::Int(val as i64) }
}

impl From<i64> for TalkValue {
    fn from(val: i64) -> TalkValue { TalkValue::Int(val) }
}

impl From<f32> for TalkValue {
    fn from(val: f32) -> TalkValue { TalkValue::Float(val as f64) }
}

impl From<f64> for TalkValue {
    fn from(val: f64) -> TalkValue { TalkValue::Float(val) }
}

impl From<&str> for TalkValue {
    fn from(val: &str) -> TalkValue { TalkValue::String(Arc::new(val.into())) }
}

impl From<&String> for TalkValue {
    fn from(val: &String) -> TalkValue { TalkValue::String(Arc::new(val.clone())) }
}

impl From<String> for TalkValue {
    fn from(val: String) -> TalkValue { TalkValue::String(Arc::new(val)) }
}

impl From<Arc<String>> for TalkValue {
    fn from(val: Arc<String>) -> TalkValue { TalkValue::String(val) }
}

impl From<&Arc<String>> for TalkValue {
    fn from(val: &Arc<String>) -> TalkValue { TalkValue::String(Arc::clone(val)) }
}

impl From<char> for TalkValue {
    fn from(val: char) -> TalkValue { TalkValue::Character(val) }
}

impl From<TalkNumber> for TalkValue {
    fn from(val: TalkNumber) -> TalkValue { 
        match val {
            TalkNumber::Int(val)    => TalkValue::Int(val),
            TalkNumber::Float(val)  => TalkValue::Float(val),
        }
    }
}

impl From<TalkError> for TalkValue {
    fn from(val: TalkError) -> TalkValue { TalkValue::Error(val) }
}

impl<T> From<Vec<T>> for TalkValue 
where
    T:          Into<TalkValue>,
{
    fn from(val: Vec<T>) -> TalkValue { TalkValue::Array(val.into_iter().map(|val| val.into()).collect()) }
}

impl TryFrom<&TalkLiteral> for TalkValue {
    type Error = TalkError;

    fn try_from(literal: &TalkLiteral) -> Result<Self, TalkError> {
        use TalkLiteral::*;

        match literal {
            Number(number)              => Self::parse_number(&*number),
            Character(chr)              => Ok(TalkValue::Character(*chr)),
            String(string)              => Ok(TalkValue::String(string.clone())),
            Symbol(symbol_name)         => Ok(TalkValue::Symbol(symbol_name.into())),
            Selector(selector_name)     => Ok(TalkValue::Selector(selector_name.into())),
            Array(values)               => values.iter().map(|value| TalkValue::try_from(value)).collect::<Result<Vec<_>, _>>().map(|values| TalkValue::Array(values)),
        }
    }
}

impl TryFrom<TalkLiteral> for TalkValue {
    type Error = TalkError;

    fn try_from(literal: TalkLiteral) -> Result<Self, TalkError> {
        TalkValue::try_from(&literal)
    }
}

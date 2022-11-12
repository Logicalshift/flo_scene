use super::context::*;
use super::continuation::*;
use super::error::*;
use super::expression::*;
use super::message::*;
use super::reference::*;
use super::symbol::*;

use std::f64;
use std::i64;
use std::u32;
use std::str::{FromStr};
use std::sync::*;

use smallvec::*;

///
/// The result of a FloTalk message
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
    Selector(TalkSymbol),

    /// An array of values
    Array(Vec<TalkValue>),

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
    /// Returns the reference represented by this value
    ///
    pub fn unwrap_as_reference(self) -> TalkReference {
        match self {
            TalkValue::Nil                  => panic!("Value is nil"),
            TalkValue::Reference(value_ref) => value_ref,
            _                               => panic!("Value is not a reference")
        }
    }

    ///
    /// Increases the reference count for this value. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn send_message_in_context(&self, message: TalkMessage, context: &mut TalkContext) -> TalkContinuation {
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
            Array(_)        |
            Error(_)        => { todo!() }

            Reference(reference) => reference.send_message_in_context(message, context),
        }
    }

    ///
    /// Increases the reference count for this value. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn add_reference(&self, context: &mut TalkContext) {
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

            Reference(reference)    => reference.add_reference(context),
            Array(values)           => values.iter().for_each(|val| val.add_reference(context)),
        }
    }

    ///
    /// Decreases the reference count for this value. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn remove_reference(&self, context: &mut TalkContext) {
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

            Reference(reference)    => reference.remove_reference(context),
            Array(values)           => values.iter().for_each(|val| val.remove_reference(context)),
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
}

impl From<()> for TalkValue {
    fn from(val: ()) -> TalkValue { TalkValue::Nil }
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

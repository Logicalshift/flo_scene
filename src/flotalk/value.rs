use super::error::*;
use super::reference::*;
use super::symbol::*;

///
/// The result of a FloTalk message
///
#[derive(Clone, PartialEq)]
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

    /// A symbol value
    Symbol(TalkSymbol),

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
}

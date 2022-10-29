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

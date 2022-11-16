use super::message::*;
use super::symbol::*;

///
/// An error 
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TalkError {
    // TODO: Error described by a FloTalk object
    // Object(TalkReference),

    /// Something is not implemented
    NotImplemented,

    /// A class message was not supported
    MessageNotSupported(TalkMessageSignatureId),

    /// The runtime was dropped before a future could completed
    RuntimeDropped,

    /// A value that was not supposed to be nil is nil
    IsNil,

    /// A value that was expected to be a reference to something was not a reference
    NotAReference,

    /// A value that was expected to be a boolean was not a boolean
    NotABoolean,

    /// A value that was expected to be a number was not a number
    NotANumber,

    /// A value that was expected to be an integer number was not an integer number
    NotAnInteger,

    /// A value that was expected to be a float was not a float
    NotAFloat,

    /// A value that looked like a floating point number could not be interpreted as such
    InvalidFloatingPointNumber(String),

    /// A value that looked like an integer number could not be interpreted as such
    InvalidIntegerNumber(String),

    /// A value that looked like a radix number could not be interpreted as such
    InvalidRadixNumber(String),

    /// A symbol was used that's not bound to any value
    UnboundSymbol(TalkSymbol),
}

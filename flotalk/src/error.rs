use super::message::*;
use super::parse_error::*;
use super::symbol::*;

///
/// A FloTalk error 
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TalkError {
    // TODO: Error described by a FloTalk object
    // Object(TalkReference),

    /// Error with parsing a script
    ParseError(TalkParseError),

    /// Something is not implemented
    NotImplemented,

    /// A class message was not supported
    MessageNotSupported(TalkMessageSignatureId),

    /// The runtime was dropped before a future could complete
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

    /// A value that was expected to be a character was not a character
    NotACharacter,

    /// Tried to send a message using something that is not a selector
    NotASelector,

    /// Tried to send a message using something that is not a symbol
    NotASymbol,

    /// Tried to send a message using something that is not a string
    NotAString,

    /// A value that was expected to be an array was not an array
    NotAnArray,

    /// A value that was expected to be a message was not a message
    NotAMessage,

    /// A value that was expected to be an error was not an error
    NotAnError,

    /// Expected an object representing a code block
    ExpectedBlockType,

    /// Expected a particular selector but not this one
    UnexpectedSelector(TalkMessageSignatureId),

    /// A value was not of an expected class type
    UnexpectedClass,

    /// A selector was called with the incorrect number of arguments
    WrongNumberOfArguments,

    /// An item (such as a selector) failed to match its expected pattern
    DoesNotMatch,

    /// A value that looked like a floating point number could not be interpreted as such
    InvalidFloatingPointNumber(String),

    /// A value that looked like an integer number could not be interpreted as such
    InvalidIntegerNumber(String),

    /// A value that looked like a radix number could not be interpreted as such
    InvalidRadixNumber(String),

    /// A symbol was used that's not bound to any value
    UnboundSymbol(TalkSymbol),

    /// A 'later' value has already been sent to the target
    AlreadySentValue,

    /// A 'later' object was dropped before it could generate a value
    NoResult,

    /// An object is already busy servicing another request so the new request cannot be processed
    Busy,

    /// A message can't be sent because the target stream is closed
    StreamClosed,

    /// A module requested for import could not be found
    ImportModuleNotFound,

    /// The value for an object was lost while something was trying to retrieve it (eg, because it was released)
    ObjectValueLost,
}

impl From<TalkParseError> for TalkError {
    fn from(parse_error: TalkParseError) -> TalkError {
        TalkError::ParseError(parse_error)
    }
}

use super::reference::*;

///
/// An error 
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TalkError {
    /// Error with a FloTalk object
    Object(TalkReference),

    /// A class message was not supported
    MessageNotSupported,

    /// The runtime was dropped before a future could completed
    RuntimeDropped,

    /// A value that looked like a floating point number could not be interpreted as such
    InvalidFloatingPointNumber(String),

    /// A value that looked like an integer number could not be interpreted as such
    InvalidIntegerNumber(String),

    /// A value that looked like a radix number could not be interpreted as such
    InvalidRadixNumber(String),
}

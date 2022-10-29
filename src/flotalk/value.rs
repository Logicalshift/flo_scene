use super::error::*;
use super::reference::*;

///
/// The result of a FloTalk message
///
pub enum TalkValue {
    /// A reference to a value
    Reference(TalkReference),

    /// A boolean value
    Bool(bool),

    /// An integer value
    Int(i64),

    /// A floating point value
    Float(f64),

    /// An error
    Error(TalkError),
}

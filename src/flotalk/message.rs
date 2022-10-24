use super::reference::*;
use super::symbol::*;

use futures::future::{BoxFuture};

///
/// Represents a flotalk message
///
pub enum TalkMessage {
    /// A message with no arguments
    Unary(TalkSymbol),

    /// A message with named arguments
    WithArguments(Vec<(TalkSymbol, TalkReference)>),
}

///
/// Raw functions return a continuation, which specifies how a result may be retrieved
///
pub enum TalkContinuation {
    /// A value that's ready now
    Ready(TalkValue),

    /// A value that is ready when a future completes
    Later(BoxFuture<'static, TalkValue>),
}

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
}

use super::reference::*;
use super::symbol::*;

///
/// Represents a flotalk message
///
pub enum TalkMessage {
    /// A message with no arguments
    Unary(TalkSymbol),

    /// A message with named arguments
    WithArguments(Vec<(TalkSymbol, TalkReference)>),
}

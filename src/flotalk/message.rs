use super::reference::*;
use super::symbol::*;

use smallvec::*;

///
/// Represents a flotalk message
///
pub enum TalkMessage {
    /// A message with no arguments
    Unary(TalkSymbol),

    /// A message with named arguments
    WithArguments(SmallVec<[(TalkSymbol, TalkReference); 4]>),
}

use super::location::*;
use super::symbol::*;

use smallvec::*;

///
/// A flattened TalkExpression (a view of a TalkExpression that operates on a stack instead of recursively)
///
/// Generic in terms of the symbol and literal value to allow for different symbol binding passes
///
#[derive(Clone, Debug)]
pub enum TalkFlatExpression<TValue, TSymbol> {
    /// Follow code comes from the specified location
    Location(TalkLocation),

    /// Load a literal value onto the stack
    Load(TValue),

    /// Load a symbol value onto the stack
    LoadFromSymbol(TSymbol),

    /// Pops an object off the stack and sends the specified message
    SendUnaryMessage(TalkSymbol),

    /// Pops message arguments and an object from the stack, and sends the specified messaage
    SendMessage(SmallVec<[TalkSymbol; 4]>),

    /// Copies the value on top of the stack
    Duplicate,

    /// Discards the value on top of the stack
    Discard,
}

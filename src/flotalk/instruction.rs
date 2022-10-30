use super::location::*;
use super::message::*;
use super::symbol::*;

///
/// A variable that is either bound to a local value, or unbound
///
#[derive(Clone, Debug)]
pub enum TalkPartialBinding {
    /// Value not bound to a local symbol
    Unbound(TalkSymbol),

    /// Bound to a local variable location (declared within the same block)
    LocalBinding(usize, TalkSymbol),
}

///
/// A single instruction for a FloTalk interpreter.
///
/// Generic in terms of the symbol and literal value to allow for different symbol binding passes
///
#[derive(Clone, Debug)]
pub enum TalkInstruction<TValue, TSymbol> {
    /// Follow code comes from the specified location
    Location(TalkLocation),

    /// Creates (or replaces) a local binding location for a symbol
    PushLocalBinding(TalkSymbol),

    /// Restores the previous binding for the specified symbol
    PopLocalBinding(TalkSymbol),

    /// Load the value indicating 'nil' to the stack
    LoadNil,

    /// Load a literal value onto the stack
    Load(TValue),

    /// Load a symbol value onto the stack
    LoadFromSymbol(TSymbol),

    /// Load an object representing a code block onto the stack
    LoadBlock(Vec<TalkSymbol>, Vec<TalkInstruction<TValue, TSymbol>>),

    /// Loads the value from the top of the stack and stores it a variable
    StoreAtSymbol(TSymbol),

    /// Pops an object off the stack and sends the specified message
    SendUnaryMessage(TalkSymbol),

    /// Pops message arguments and an object from the stack, and sends the specified message, leaving the result on the stack. Number of arguments is supplied, and must match the number in the message signature.
    SendMessage(TalkMessageSignatureId, usize),

    /// Copies the value on top of the stack
    Duplicate,

    /// Discards the value on top of the stack
    Discard,
}

use super::reference::*;
use super::symbol::*;

use smallvec::*;

///
/// Represents a flotalk message
///
#[derive(Clone)]
pub enum TalkMessage {
    /// A message with no arguments
    Unary(TalkSymbol),

    /// A message with named arguments
    WithArguments(SmallVec<[(TalkSymbol, TalkReference); 4]>),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TalkMessageSignature {
    Unary(TalkSymbol),
    Arguments(SmallVec<[TalkSymbol; 4]>),
}

impl TalkMessage {
    ///
    /// Converts a message to its signature
    ///
    #[inline]
    pub fn signature(&self) -> TalkMessageSignature {
        match self {
            TalkMessage::Unary(symbol)          => TalkMessageSignature::Unary(*symbol),
            TalkMessage::WithArguments(args)    => TalkMessageSignature::Arguments(args.iter().map(|(sym, _)| *sym).collect())
        }
    }
}
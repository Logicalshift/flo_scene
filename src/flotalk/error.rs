use super::reference::*;

///
/// An error 
///
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TalkError {
    /// Error with a FloTalk object
    Object(TalkReference),

    /// The runtime was dropped before a future could completed
    RuntimeDropped,
}

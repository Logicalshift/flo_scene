///
/// A parser error in a flotalk program
///
#[derive(Clone, PartialEq, Debug)]
pub enum TalkParseError {
    /// A fallback error for when we don't have a specific cause of the issue
    GenericError,
}

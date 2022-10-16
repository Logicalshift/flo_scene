///
/// A parser error in a flotalk program
///
#[derive(Clone, PartialEq, Debug)]
pub enum TalkParseError {
    /// A fallback error for when we don't have a specific cause of the issue
    GenericError,

    /// A character was unexpected (with an optional list of expected characters at this point)
    UnexpectedCharacter(char),

    /// A '"' comment had no closing '"'
    UnclosedDoubleQuoteComment,

    /// Unexpected end of stream
    ExpectedMoreCharacters,
}

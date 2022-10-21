///
/// A parser error in a flotalk program
///
#[derive(Clone, PartialEq, Debug)]
pub enum TalkParseError {
    /// A fallback error for when we don't have a specific cause of the issue
    GenericError,

    /// The parser encountered a failure due to inconsistent state
    InconsistentState,

    /// A character was unexpected (with an optional list of expected characters at this point)
    UnexpectedCharacter(char),

    /// A '"' comment had no closing '"'
    UnclosedDoubleQuoteComment,

    /// A '#' was not followed by a valid character for declaring an array or a symbol
    NotAnArrayOrSymbol,

    /// A ')' was not found for an bracketed expression
    MissingCloseBracket,

    /// Unexpected end of stream
    ExpectedMoreCharacters,

    /// A keyword (`foo:`) was used where an identifier (`foo`) was expected
    KeywordNotValidHere,

    /// The RHS of a binary expression does not seem to be valid
    MissingRValue,

    /// The argument of a keyword message is missing
    MissingMessageArgument,
}

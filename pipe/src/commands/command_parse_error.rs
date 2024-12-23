use crate::parser::*;

///
/// The errors that can happen while parsing a command
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandParseError {
    /// Error while parsing a JSON argument
    JsonError(JsonParseError),

    /// The lookahead wasn't expected at this point
    UnexpectedToken(Option<CommandToken>, String),

    /// Ran out of input while parsing the command
    ExpectedMoreInput,

    /// Usually an error in the parser, we tried to 'reduce' a token when we hadn't previously accepted enough input 
    ParserStackTooSmall,

    /// Usually indicates an error with the parser, we failed to 'converge' to a single value
    ParserDidNotConverge,
}

impl<'a, TToken> From<&'a TokenMatch<TToken>> for CommandParseError 
where
    TToken: Clone + TryInto<CommandToken>,
{
    fn from(err_lookahead: &'a TokenMatch<TToken>) -> CommandParseError {
        let json_token = err_lookahead.token.clone().map(|token| token.try_into());

        match json_token {
            Some(token) => CommandParseError::UnexpectedToken(token.ok(), err_lookahead.fragment.clone()),
            None        => CommandParseError::UnexpectedToken(None, err_lookahead.fragment.clone()),
        }
    }
}

impl From<ParserLookaheadEmpty> for CommandParseError {
    fn from(_err: ParserLookaheadEmpty) -> CommandParseError {
        CommandParseError::ExpectedMoreInput
    }
}

impl From<ParserStackTooSmall> for CommandParseError {
    fn from(_err: ParserStackTooSmall) -> CommandParseError {
        CommandParseError::ParserStackTooSmall
    }
}

impl From<ParserDidNotConverge> for CommandParseError {
    fn from(_err: ParserDidNotConverge) -> CommandParseError {
        CommandParseError::ParserDidNotConverge
    }
}

impl From<JsonParseError> for CommandParseError {
    fn from(err: JsonParseError) -> CommandParseError {
        CommandParseError::JsonError(err)
    }
}

impl From<CommandParseError> for JsonParseError {
    fn from(err: CommandParseError) -> JsonParseError {
        match err {
            CommandParseError::JsonError(err)               => err,
            CommandParseError::UnexpectedToken(token, msg)  => JsonParseError::UnexpectedToken(token.and_then(|token| token.try_into().ok()), msg),
            CommandParseError::ExpectedMoreInput            => JsonParseError::CommandExpectedMoreInput,
            CommandParseError::ParserStackTooSmall          => JsonParseError::ParserStackTooSmall,
            CommandParseError::ParserDidNotConverge         => JsonParseError::ParserDidNotConverge,

        }
    }
}

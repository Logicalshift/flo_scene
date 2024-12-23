use crate::parser::*;
use crate::parse_json::{JsonToken};

///
/// Errors that can occur while parsing JSON
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum JsonParseError {
    /// The lookahead wasn't expected at this point
    UnexpectedToken(Option<JsonToken>, String),

    /// Expected a ':' character, but got something else
    ExpectedColon(Option<JsonToken>, String),

    /// The parser succeded in matching the input, but more was expected
    ExpectedMoreInput(JsonInputType),

    /// Usually an error in the parser, we tried to 'reduce' a token when we hadn't previously accepted enough input 
    ParserStackTooSmall,

    /// A value that the parser thought was valid JSON was rejected by serde (usually indicating an error in this parser)
    SerdeJsonError,

    /// Usually indicates an error with the parser, we failed to 'converge' to a single value
    ParserDidNotConverge,

    /// Expected more input while parsing a substituted command
    CommandExpectedMoreInput,
}

impl<'a, TToken> From<&'a TokenMatch<TToken>> for JsonParseError 
where
    TToken: Clone + TryInto<JsonToken>,
{
    fn from(err_lookahead: &'a TokenMatch<TToken>) -> JsonParseError {
        let json_token = err_lookahead.token.clone().map(|token| token.try_into());

        match json_token {
            Some(token) => JsonParseError::UnexpectedToken(token.ok(), err_lookahead.fragment.clone()),
            None        => JsonParseError::UnexpectedToken(None, err_lookahead.fragment.clone()),
        }
    }
}

impl<'a, TToken> From<ExpectedTokenError<'a, TokenMatch<TToken>>> for JsonParseError 
where
    TToken: Clone + TryInto<JsonToken>,
{
    fn from(err_expected_token: ExpectedTokenError<'a, TokenMatch<TToken>>) -> JsonParseError {
        match err_expected_token {
            ExpectedTokenError::ParserLookaheadEmpty        => JsonParseError::ExpectedMoreInput(JsonInputType::LookaheadEmpty),
            ExpectedTokenError::UnexpectedToken(lookahead)  => {
                let json_token = lookahead.token.clone().map(|token| token.try_into());

                match json_token {
                    Some(token) => JsonParseError::UnexpectedToken(token.ok(), lookahead.fragment.clone()),
                    None        => JsonParseError::UnexpectedToken(None, lookahead.fragment.clone()),
                }
            }
        }
    }
}

impl From<ParserLookaheadEmpty> for JsonParseError {
    fn from(_err: ParserLookaheadEmpty) -> JsonParseError {
        JsonParseError::ExpectedMoreInput(JsonInputType::LookaheadEmpty)
    }
}

impl From<ParserStackTooSmall> for JsonParseError {
    fn from(_err: ParserStackTooSmall) -> JsonParseError {
        JsonParseError::ParserStackTooSmall
    }
}

impl From<serde_json::Error> for JsonParseError {
    fn from(_err: serde_json::Error) -> JsonParseError {
        JsonParseError::SerdeJsonError
    }
}

impl From<ParserDidNotConverge> for JsonParseError {
    fn from(_err: ParserDidNotConverge) -> JsonParseError {
        JsonParseError::ParserDidNotConverge
    }
}

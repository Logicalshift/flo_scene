use crate::parser::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use itertools::*;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap};

///
/// The tokens that make up the JSON language
///
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum JsonToken {
    Whitespace,
    Number,
    String,
    Variable,
    True,
    False,
    Null,
    Character(char),
}

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
}

///
/// The parsed form of a JSON statement. This can incorporate variables, as well as the parts of a standard JSON value
///
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ParsedJson {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<ParsedJson>),
    Object(HashMap<String, ParsedJson>),
    Variable(String),
}

///
/// What type of 'more' input was expected
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum JsonInputType {
    LookaheadEmpty,
    StartOfValue,
    StartOfObject,
    ObjectValues,
    AfterObjectValue,
    StartOfArray,
    ArrayValues,
    AfterArrayValue,
    String,
    Number,
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

/// Matches a string against the JSON whitespace syntax
pub (crate) fn match_whitespace(lookahead: &str, eof: bool) -> TokenMatchResult<JsonToken> {
    // "^(([ \t]*[\r\n])|([ \t]+))"
    let mut chrs = lookahead.chars();

    if let Some(chr) = chrs.next() {
        if !chr.is_whitespace() {
            // First character must be a whitespace character
            TokenMatchResult::LookaheadCannotMatch
        } else if chr == '\n' || chr == '\r' {
            // We end early on newlines to allow for interactive parsing
            TokenMatchResult::Matches(JsonToken::Whitespace, 1)
        } else {
            let mut len = 1;

            while let Some(chr) = chrs.next() {
                if chr == '\n' || chr == '\r' {
                    // We end early on newlines to allow for interactive parsing
                    return TokenMatchResult::Matches(JsonToken::Whitespace, len + 1);
                }

                if !chr.is_whitespace() {
                    return TokenMatchResult::Matches(JsonToken::Whitespace, len);
                }

                len += 1;
            }

            if eof {
                TokenMatchResult::Matches(JsonToken::Whitespace, len)
            } else {
                TokenMatchResult::LookaheadIsPrefix
            }
        }
    } else {
        // Zero length string is prefix of anything
        TokenMatchResult::LookaheadIsPrefix
    }
}

/// Matches a string against the JSON number syntax
#[inline]
fn match_number(lookahead: &str, eof: bool) -> TokenMatchResult<JsonToken> {
    // "^(-)?[0-9]+(\\.[0-9]+)?([eE]([+-])?[0-9]+)?"
    let mut chrs = lookahead.chars();

    // Start with a '-' or a digit
    let mut maybe_chr = chrs.next();
    if let Some(chr) = maybe_chr {
        if chr != '-' && !chr.is_digit(10) { return TokenMatchResult::LookaheadCannotMatch };

        // Here's where a goto instruction would be useful as this is really a state machine
        let mut len = 1;
        if chr == '-' {
            maybe_chr = chrs.next();

            if let Some(chr) = maybe_chr {
                // Character after a '-' must be a digit
                if !chr.is_digit(10) {
                    return TokenMatchResult::LookaheadCannotMatch;
                }

                len += 1;
            } else if !eof {
                // '-' not folowed by anything
                return TokenMatchResult::LookaheadIsPrefix;
            } else {
                return TokenMatchResult::LookaheadCannotMatch;
            }
        }

        // Integer portion
        loop {
            maybe_chr = chrs.next();
            if let Some(chr) = maybe_chr {
                if chr.is_digit(10) {
                    len += 1;
                    continue;
                } else {
                    break;
                }
            } else if !eof {
                return TokenMatchResult::LookaheadIsPrefix;
            } else {
                return TokenMatchResult::Matches(JsonToken::Number, len);
            }
        }

        // Decimal portion
        if maybe_chr == Some('.') {
            len += 1;

            loop {
                maybe_chr = chrs.next();

                if let Some(chr) = maybe_chr {
                    if chr.is_digit(10) {
                        len += 1;
                        continue;
                    } else {
                        break;
                    }
                } else if !eof {
                    return TokenMatchResult::LookaheadIsPrefix;
                } else {
                    return TokenMatchResult::Matches(JsonToken::Number, len);
                }
            }
        }

        // Exponent portion
        if maybe_chr == Some('e') || maybe_chr == Some('E') {
            // Followed by a '+' or a '-' or a digit
            maybe_chr = chrs.next();

            if let Some(chr) = maybe_chr {
                // Is a number if it's 'e<notvalid>'
                if chr != '+' && chr != '-' && !chr.is_digit(10) { return TokenMatchResult::Matches(JsonToken::Number, len); }

                len += 2;

                // Match digits
                loop {
                    maybe_chr = chrs.next();

                    if let Some(chr) = maybe_chr {
                        if chr.is_digit(10) {
                            len += 1;
                            continue;
                        } else {
                            break;
                        }
                    } else if !eof {
                        return TokenMatchResult::LookaheadIsPrefix;
                    } else {
                        return TokenMatchResult::Matches(JsonToken::Number, len);
                    }
                }
            } else if !eof {
                return TokenMatchResult::LookaheadIsPrefix;
            } else {
                return TokenMatchResult::Matches(JsonToken::Number, len);
            }
        }

        if maybe_chr.is_some() {
            TokenMatchResult::Matches(JsonToken::Number, len)
        } else if !eof {
            TokenMatchResult::LookaheadIsPrefix
        } else {
            TokenMatchResult::Matches(JsonToken::Number, len)
        }
    } else {
        // A 0 length string is a prefix for anything
        TokenMatchResult::LookaheadIsPrefix
    }
}

/// Matches a string against the JSON string syntax
#[inline]
fn match_string(lookahead: &str, eof: bool) -> TokenMatchResult<JsonToken> {
    // r#"^"([^"\\]|(\\["\\/bfnrtu]))*""#
    let mut chrs = lookahead.chars();

    // First character must be a '"'
    if let Some(chr) = chrs.next() {
        if chr != '\"' { return TokenMatchResult::LookaheadCannotMatch; }

        let mut len = 1;
        while let Some(chr) = chrs.next() {
            if chr == '\"' {
                // Closing quote
                return TokenMatchResult::Matches(JsonToken::String, len + 1);
            }

            if chr == '\\' {
                // Quoted character
                if let Some(_quoted) = chrs.next() {
                    len += 1;
                } else if !eof {
                    return TokenMatchResult::LookaheadIsPrefix;
                } else {
                    return TokenMatchResult::LookaheadCannotMatch;
                }
            }

            len += 1;
        }

        // Cannot be a string if we reach the end of file
        if !eof {
            TokenMatchResult::LookaheadIsPrefix
        } else {
            TokenMatchResult::LookaheadCannotMatch
        }
    } else {
        // Zero length string is prefix of anything
        TokenMatchResult::LookaheadIsPrefix
    }
}

/// Matches a string against the 'true' keyword
fn match_true(lookahead: &str, _eof: bool) -> TokenMatchResult<JsonToken> {
    if lookahead.len() < 4 {
        if lookahead == &"true"[0..lookahead.len()] {
            TokenMatchResult::LookaheadIsPrefix
        } else {
            TokenMatchResult::LookaheadCannotMatch
        }
    } else if &lookahead[0..4] == "true" {
        TokenMatchResult::Matches(JsonToken::True, 4)
    } else {
        TokenMatchResult::LookaheadCannotMatch
    }
}

/// Matches a string against the 'false' keyword
fn match_false(lookahead: &str, _eof: bool) -> TokenMatchResult<JsonToken> {
    if lookahead.len() < 5 {
        if lookahead == &"false"[0..lookahead.len()] {
            TokenMatchResult::LookaheadIsPrefix
        } else {
            TokenMatchResult::LookaheadCannotMatch
        }
    } else if &lookahead[0..5] == "false" {
        TokenMatchResult::Matches(JsonToken::False, 5)
    } else {
        TokenMatchResult::LookaheadCannotMatch
    }
}

/// Matches a string against the 'null' keyword
fn match_null(lookahead: &str, _eof: bool) -> TokenMatchResult<JsonToken> {
    if lookahead.len() < 4 {
        if lookahead == &"null"[0..lookahead.len()] {
            TokenMatchResult::LookaheadIsPrefix
        } else {
            TokenMatchResult::LookaheadCannotMatch
        }
    } else if &lookahead[0..4] == "null" {
        TokenMatchResult::Matches(JsonToken::Null, 4)
    } else {
        TokenMatchResult::LookaheadCannotMatch
    }
}

/// Matches any character
fn match_character(lookahead: &str, _eof: bool) -> TokenMatchResult<JsonToken> {
    if let Some(chr) = lookahead.chars().next() {
        TokenMatchResult::Matches(JsonToken::Character(chr), 1)
    } else {
        TokenMatchResult::LookaheadCannotMatch
    }
}

impl<TToken> TokenMatcher<TToken> for JsonToken 
where
    TToken: From<JsonToken>,
{
    fn try_match(&self, lookahead: &'_ str, eof: bool) -> TokenMatchResult<TToken> {
        use JsonToken::*;

        match self {
            Whitespace      => match_whitespace(lookahead, eof).into(),
            Number          => match_number(lookahead, eof).into(),
            Character(_)    => match_character(lookahead, eof).into(),
            String          => match_string(lookahead, eof).into(),
            True            => match_true(lookahead, eof).into(),
            False           => match_false(lookahead, eof).into(),
            Null            => match_null(lookahead, eof).into(),
            Variable        => TokenMatchResult::LookaheadCannotMatch, // These are generated externally, so there's no matcher here
        }
    }
}

impl<TToken, TStream> Tokenizer<TToken, TStream> 
where
    TToken: From<JsonToken>,
{
    ///
    /// Adds the set of JSON token matchers to this tokenizer
    ///
    pub fn with_json_matchers(&mut self) -> &mut Self {
        self
            .with_matcher(JsonToken::Character(' '))
            .with_matcher(JsonToken::Whitespace)
            .with_matcher(JsonToken::Number)
            .with_matcher(JsonToken::String)
            .with_matcher(JsonToken::True)
            .with_matcher(JsonToken::False)
            .with_matcher(JsonToken::Null)
    }
}

///
/// Reads a JSON token from the tokenizer
///
pub async fn json_read_token<TStream, TToken>(tokenizer: &mut Tokenizer<TToken, TStream>) -> Option<TokenMatch<TToken>>
where
    TStream:    Send + Stream<Item=Vec<u8>>,
    TToken:     Clone + Send + TryInto<JsonToken>,
{
    loop {
        // Acquire a token from the tokenizer
        let next_match = tokenizer.match_token().await?;

        // Skip over whitespace, then return the first 'sold' value
        match next_match.token.clone()?.try_into() {
            Ok(JsonToken::Whitespace)   => { }
            Ok(_)                       => { break Some(next_match); }
            Err(_)                      => { break Some(next_match); }
        }
    }
}

///
/// Attempts to parse a JSON value starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub fn json_parse_value<'a, TStream, TToken>(parser: &'a mut Parser<TokenMatch<TToken>, ParsedJson>, tokenizer: &'a mut Tokenizer<TToken, TStream>) -> BoxFuture<'a, Result<(), JsonParseError>>
where
    TStream:        Send + Stream<Item=Vec<u8>>,
    TToken:         Clone + Send + TryInto<JsonToken>,
    TToken::Error:  Send
{
    async move {
        let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;

        if let Some(lookahead) = lookahead {
            // Decide which matcher to use based on the lookahead
            let json_token = lookahead.token.clone().map(|token| token.try_into());

            match json_token {
                Some(Ok(JsonToken::String))         => json_parse_string(parser, tokenizer).await,
                Some(Ok(JsonToken::Number))         => json_parse_number(parser, tokenizer).await,
                Some(Ok(JsonToken::Character('{'))) => json_parse_object(parser, tokenizer).await,
                Some(Ok(JsonToken::Character('['))) => json_parse_array(parser, tokenizer).await,
                Some(Ok(JsonToken::True))           => { parser.accept_token()?.reduce(1, |_| ParsedJson::Bool(true))?; Ok(()) },
                Some(Ok(JsonToken::False))          => { parser.accept_token()?.reduce(1, |_| ParsedJson::Bool(false))?; Ok(()) },
                Some(Ok(JsonToken::Null))           => { parser.accept_token()?.reduce(1, |_| ParsedJson::Null)?; Ok(()) },
                _                                   => Err(lookahead.into())
            }
        } else {
            // Error if there's no symbol
            Err(JsonParseError::ExpectedMoreInput(JsonInputType::StartOfValue))
        }
    }.boxed()
}

///
/// Attempts to parse a JSON object starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub async fn json_parse_object<TStream, TToken>(parser: &mut Parser<TokenMatch<TToken>, ParsedJson>, tokenizer: &mut Tokenizer<TToken, TStream>) -> Result<(), JsonParseError>
where
    TStream:        Send + Stream<Item=Vec<u8>>,
    TToken:         Clone + Send + TryInto<JsonToken>,
    TToken::Error:  Send
{
    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;
    let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(JsonParseError::ExpectedMoreInput(JsonInputType::StartOfObject)) };

    if let Some(Ok(JsonToken::Character('{'))) = lookahead.token.clone().map(|token| token.try_into()) {
        // Accept the initial '{'
        parser.accept_token()?;

        let mut num_tokens = 1;

        loop {
            // Look to the next value to decide what to do
            let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;
            let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(JsonParseError::ExpectedMoreInput(JsonInputType::ObjectValues)) };

            match lookahead.token.clone().map(|token| token.try_into()) {
                Some(Ok(JsonToken::Character('}'))) => {
                    // '}' Finishes the object successfully
                    parser.accept_token()?;
                    num_tokens += 1;
                    break;
                },

                Some(Ok(JsonToken::String)) => {
                    // <String> : <Value>
                    parser.accept_token()?;
                    parser.reduce(1, |string| serde_json::from_str(&string[0].token().unwrap().fragment).unwrap())?;
                    num_tokens += 1;

                    // ... ':'
                    parser.ensure_lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;
                    parser.accept_expected_token(|token| {
                        if let Some(Ok(JsonToken::Character(':'))) = token.token.clone().map(|token| token.try_into()) {
                            true
                        } else {
                            false
                        }
                    })?;
                    num_tokens += 1;

                    json_parse_value(parser, tokenizer).await?;
                    num_tokens += 1;

                    // ',' or '}'
                    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;
                    let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(JsonParseError::ExpectedMoreInput(JsonInputType::AfterObjectValue)) };

                    let json_token = lookahead.token.clone().map(|token| token.try_into());
                    match json_token {
                        Some(Ok(JsonToken::Character('}'))) => {
                            // Ends the object
                            parser.accept_token()?;
                            num_tokens += 1;
                            break;
                        }

                        Some(Ok(JsonToken::Character(','))) => {
                            // More fields
                            parser.accept_token()?;
                            num_tokens += 1;
                        }

                        _ => {
                            // Unexpected value
                            return Err(lookahead.into());
                        }
                    }
                },

                _ => {
                    return Err(lookahead.into());
                }
            }
        }

        // Reduce the object to a value
        parser.reduce(num_tokens, |fields| {
            let values = fields.into_iter()
                .skip(1)
                .tuples()
                .map(|(key, _colon, value, _comma_or_brace)| {
                    // Key should be a string node
                    let key = match key.to_node() {
                        Some(ParsedJson::String(key))   => key,
                        _                               => panic!(),
                    };

                    (key, value.to_node().unwrap())
                });

            ParsedJson::Object(values.collect())
        })?;

        Ok(())
    } else {
        // Doesn't start with '{'
        Err(lookahead.into())
    }
}

///
/// Attempts to parse a JSON array starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub async fn json_parse_array<TStream, TToken>(parser: &mut Parser<TokenMatch<TToken>, ParsedJson>, tokenizer: &mut Tokenizer<TToken, TStream>) -> Result<(), JsonParseError>
where
    TStream:        Send + Stream<Item=Vec<u8>>,
    TToken:         Clone + Send + TryInto<JsonToken>,
    TToken::Error:  Send
{
    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;
    let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(JsonParseError::ExpectedMoreInput(JsonInputType::StartOfArray)) };

    if let Some(Ok(JsonToken::Character('['))) = lookahead.token.clone().map(|token| token.try_into()) {
        // Accept the initial '['
        parser.accept_token()?;

        let mut num_tokens = 1;


        loop {
            // Look ahead to the next value
            let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;
            let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(JsonParseError::ExpectedMoreInput(JsonInputType::ArrayValues)) };

            // ']' to finish the array, or else a JSON value
            match lookahead.token.clone().map(|token| token.try_into()) {
                Some(Ok(JsonToken::Character(']'))) => {
                    // Accept the ']'
                    parser.accept_token()?;
                    num_tokens += 1;

                    // Successfully parsed the array
                    break;
                }

                _ => {
                    // Read the next value
                    json_parse_value(parser, tokenizer).await?;
                    num_tokens += 1;

                    // Next token should be a ',' for more array or ']' for the end of the array
                    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;
                    let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(JsonParseError::ExpectedMoreInput(JsonInputType::AfterArrayValue)) };

                    match lookahead.token.clone().map(|token| token.try_into()) {
                        Some(Ok(JsonToken::Character(','))) => {
                            // Continues the array
                            parser.accept_token()?;
                            num_tokens += 1;
                        }

                        Some(Ok(JsonToken::Character(']'))) => {
                            // Finishes the array
                            parser.accept_token()?;
                            num_tokens += 1;
                            break;
                        }

                        _ => {
                            // Invalid value
                            return Err(lookahead.into());
                        }
                    }
                }
            }
        }

        // Reduce the array to a value
        parser.reduce(num_tokens, |values| {
            let values = values.into_iter()
                .skip(1)
                .tuples()
                .map(|(value, _comma_or_bracket)| value.to_node().unwrap());

            ParsedJson::Array(values.collect())
        })?;

        Ok(())
    } else {
        // Not an array
        Err(lookahead.into())
    }
}

///
/// Attempts to parse a JSON string starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub async fn json_parse_string<TStream, TToken>(parser: &mut Parser<TokenMatch<TToken>, ParsedJson>, tokenizer: &mut Tokenizer<TToken, TStream>) -> Result<(), JsonParseError>
where
    TStream:        Send + Stream<Item=Vec<u8>>,
    TToken:         Clone + Send + TryInto<JsonToken>,
    TToken::Error:  Send
{
    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;

    if let Some(lookahead) = lookahead {
        if let Some(Ok(JsonToken::String)) = lookahead.token.clone().map(|token| token.try_into()) {
            // Reduce as a string
            let value = serde_json::from_str(&lookahead.fragment)?;

            parser.accept_token()?.reduce(1, |_| value)?;
            Ok(())
        } else {
            // Not a string
            Err(lookahead.into())
        }
    } else {
        // No lookahead
        Err(JsonParseError::ExpectedMoreInput(JsonInputType::String))
    }
}

///
/// Attempts to parse a JSON object starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub async fn json_parse_number<TStream, TToken>(parser: &mut Parser<TokenMatch<TToken>, ParsedJson>, tokenizer: &mut Tokenizer<TToken, TStream>) -> Result<(), JsonParseError>
where
    TStream:        Send + Stream<Item=Vec<u8>>,
    TToken:         Clone + Send + TryInto<JsonToken>,
    TToken::Error:  Send
{
    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed()).await;

    if let Some(lookahead) = lookahead {
        if let Some(Ok(JsonToken::Number)) = lookahead.token.clone().map(|token| token.try_into()) {
            // Reduce as a number
            let value = serde_json::from_str(&lookahead.fragment)?;

            parser.accept_token()?.reduce(1, |_| value)?;
            Ok(())
        } else {
            // Not a number
            Err(lookahead.into())
        }
    } else {
        // No lookahead
        Err(JsonParseError::ExpectedMoreInput(JsonInputType::Number))
    }
}

impl Into<serde_json::Value> for ParsedJson {
    ///
    /// Converts a parsed JSON structure into a serde value. All variables are replaced with 'null':
    /// if you want to substitute these you will need to implement a separate converter.
    ///
    fn into(self) -> serde_json::Value {
        use ParsedJson::*;
        use serde_json::Value;

        match self {
            Variable(_)     => Value::Null,

            Null            => Value::Null,
            Bool(val)       => Value::Bool(val),
            Number(num)     => Value::Number(num),
            String(string)  => Value::String(string),
            Array(array)    => Value::Array(array.into_iter().map(|val| val.into()).collect()),
            Object(map)     => Value::Object(map.into_iter().map(|(key, val)| (key, val.into())).collect()),
        }
    }
}

impl ParsedJson {
    ///
    /// Returns this object as a serde_json value, with any variables substituted with 'null'.
    ///
    #[inline]
    pub fn to_serde(self) -> serde_json::Value {
        self.into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use futures::executor;
    use futures::stream;

    #[test]
    pub fn reject_not_a_number() {
        let match_result = match_number("erg", true);
        assert!(match_result == TokenMatchResult::LookaheadCannotMatch, "{:?}", match_result);
    }

    #[test]
    pub fn reject_not_a_string() {
        let match_result = match_string("1234", true);
        assert!(match_result == TokenMatchResult::LookaheadCannotMatch, "{:?}", match_result);
    }

    #[test]
    pub fn reject_not_a_number_negative() {
        let match_result = match_number("-erg", true);
        assert!(match_result == TokenMatchResult::LookaheadCannotMatch, "{:?}", match_result);
    }

    #[test]
    pub fn reject_not_a_number_suffix() {
        let match_result = match_number("er1234", true);
        assert!(match_result == TokenMatchResult::LookaheadCannotMatch, "{:?}", match_result);
    }

    #[test]
    pub fn match_simple_number() {
        let match_result = match_number("1234", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_number_prefix() {
        let match_result = match_number("1234rG", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_number_prefix_e() {
        let match_result = match_number("1234ErG", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_decimal_number() {
        let match_result = match_number("12.34", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 5), "{:?}", match_result);
    }

    #[test]
    pub fn match_negative_number() {
        let match_result = match_number("-1234", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 5), "{:?}", match_result);
    }

    #[test]
    pub fn partial_match_number() {
        let match_result = match_number("1234", false);
        assert!(match_result == TokenMatchResult::LookaheadIsPrefix, "{:?}", match_result);
    }

    #[test]
    pub fn match_number_with_following_data_1() {
        let match_result = match_number("1234  ", false);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_number_with_following_data_2() {
        let match_result = match_number("1234 ", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_number_with_following_data_3() {
        let match_result = match_number("1 1234", false);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 1), "{:?}", match_result);
    }

    #[test]
    pub fn match_number_with_following_data_4() {
        let match_result = match_number("1234 1234", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_number_with_following_data_5() {
        let match_result = match_number("-1 1234", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 2), "{:?}", match_result);
    }

    #[test]
    pub fn match_number_with_following_number() {
        let match_result = match_number("1234 12345", false);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_exponent_number() {
        let match_result = match_number("12e34", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 5), "{:?}", match_result);
    }

    #[test]
    pub fn match_decimal_exponent_number() {
        let match_result = match_number("12.34e56", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 8), "{:?}", match_result);
    }

    #[test]
    pub fn match_empty_string() {
        let match_result = match_string(r#""""#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::String, 2), "{:?}", match_result);
    }

    #[test]
    pub fn match_partial_string_1() {
        let match_result = match_string(r#"""#, false);
        assert!(match_result == TokenMatchResult::LookaheadIsPrefix, "{:?}", match_result);
    }

    #[test]
    pub fn match_partial_string_2() {
        let match_result = match_string(r#""partial"#, false);
        assert!(match_result == TokenMatchResult::LookaheadIsPrefix, "{:?}", match_result);
    }

    #[test]
    pub fn eof_is_not_partial_string() {
        let match_result = match_string(r#""partial"#, true);
        assert!(match_result == TokenMatchResult::LookaheadCannotMatch, "{:?}", match_result);
    }

    #[test]
    pub fn match_basic_string() {
        let match_result = match_string(r#""Hello, world""#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::String, 14), "{:?}", match_result);
    }

    #[test]
    pub fn match_unicode_string() {
        let match_result = match_string(r#""𐍈""#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::String, 3), "{:?}", match_result);
    }

    #[test]
    pub fn match_quoted_string() {
        let match_result = match_string(r#""\"""#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::String, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_other_quotes() {
        let match_result = match_string(r#""\\\n\r\b\t\f\/\uabcd""#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::String, 22), "{:?}", match_result);
    }

    #[test]
    pub fn match_string_with_following_data() {
        let match_result = match_string(r#""Hello, world" with following data"#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::String, 14), "{:?}", match_result);
    }

    #[test]
    pub fn match_true_full() {
        let match_result = match_true(r#"true"#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::True, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_true_prefix() {
        let match_result = match_true(r#"tru"#, true);
        assert!(match_result == TokenMatchResult::LookaheadIsPrefix, "{:?}", match_result);
    }

    #[test]
    pub fn match_false_full() {
        let match_result = match_false(r#"false"#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::False, 5), "{:?}", match_result);
    }

    #[test]
    pub fn match_false_prefix() {
        let match_result = match_false(r#"fal"#, true);
        assert!(match_result == TokenMatchResult::LookaheadIsPrefix, "{:?}", match_result);
    }

    #[test]
    pub fn match_null_full() {
        let match_result = match_null(r#"null"#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Null, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_null_prefix() {
        let match_result = match_null(r#"nul"#, true);
        assert!(match_result == TokenMatchResult::LookaheadIsPrefix, "{:?}", match_result);
    }

    #[test]
    pub fn match_whitespace_with_following_data() {
        let match_result = match_whitespace(r#"    1234"#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Whitespace, 4), "{:?}", match_result);
    }

    #[test]
    pub fn match_whitespace_stops_at_newline() {
        let match_result = match_whitespace("  \n  1234", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Whitespace, 3), "{:?}", match_result);
    }

    #[test]
    pub fn rejects_not_whitespace() {
        let match_result = match_whitespace(r#"1 234"#, true);
        assert!(match_result == TokenMatchResult::LookaheadCannotMatch, "{:?}", match_result);
    }

    #[test]
    pub fn match_whitespace_prefix() {
        let match_result = match_whitespace(r#"  "#, false);
        assert!(match_result == TokenMatchResult::LookaheadIsPrefix, "{:?}", match_result);
    }

    #[test]
    pub fn match_whitespace_eof() {
        let match_result = match_whitespace(r#"  "#, true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Whitespace, 2), "{:?}", match_result);
    }

    #[test]
    pub fn json_tokenizer() {
        // Input stream with all the JSON token types
        let input           = r#"1 1234 1234.4 -24 "string" true false null { } "#;

        // Create a JSON tokenizer
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(input.bytes()).ready_chunks(2));
        tokenizer.with_json_matchers();

        // Tokenize all the symbols
        executor::block_on(async {
            let num1 = tokenizer.match_token().await.unwrap();
            assert!(num1.fragment == "1", "Fragment is {:?} (should be '1')", num1);
            assert!(num1.token == Some(JsonToken::Number), "Token is {:?} (should be Number)", num1.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let num2 = tokenizer.match_token().await.unwrap();
            assert!(num2.fragment == "1234", "Fragment is {:?} (should be '1234')", num2);
            assert!(num2.token == Some(JsonToken::Number), "Token is {:?} (should be Number)", num2.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let num3 = tokenizer.match_token().await.unwrap();
            assert!(num3.fragment == "1234.4", "Fragment is {:?} (should be '1234.4')", num3);
            assert!(num3.token == Some(JsonToken::Number), "Token is {:?} (should be Number)", num3.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let num4 = tokenizer.match_token().await.unwrap();
            assert!(num4.fragment == "-24", "Fragment is {:?} (should be '-24')", num4);
            assert!(num4.token == Some(JsonToken::Number), "Token is {:?} (should be Number)", num4.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let string = tokenizer.match_token().await.unwrap();
            assert!(string.fragment == "\"string\"", "Fragment is {:?} (should be '\"string\"')", string);
            assert!(string.token == Some(JsonToken::String), "Token is {:?} (should be String)", string.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let token = tokenizer.match_token().await.unwrap();
            assert!(token.fragment == "true", "Fragment is {:?} (should be 'true')", token);
            assert!(token.token == Some(JsonToken::True), "Token is {:?} (should be True)", token.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let token = tokenizer.match_token().await.unwrap();
            assert!(token.fragment == "false", "Fragment is {:?} (should be 'false')", token);
            assert!(token.token == Some(JsonToken::False), "Token is {:?} (should be Fa)", token.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let token = tokenizer.match_token().await.unwrap();
            assert!(token.fragment == "null", "Fragment is {:?} (should be 'null')", token);
            assert!(token.token == Some(JsonToken::Null), "Token is {:?} (should be Null)", token.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let token = tokenizer.match_token().await.unwrap();
            assert!(token.fragment == "{", "Fragment is {:?} (should be '{{')", token);
            assert!(token.token == Some(JsonToken::Character('{')), "Token is {:?} (should be Character)", token.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            let token = tokenizer.match_token().await.unwrap();
            assert!(token.fragment == "}", "Fragment is {:?} (should be '}}')", token);
            assert!(token.token == Some(JsonToken::Character('}')), "Token is {:?} (should be Character)", token.token);

            assert!(tokenizer.match_token().await.unwrap().token == Some(JsonToken::Whitespace), "Not followed by whitespace");

            assert!(tokenizer.match_token().await == None, "Final token not None");
        });
    }

    #[test]
    pub fn parse_string() {
        let test_value      = r#" "string" "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_string(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == serde_json::Value::String("string".into()));
        })
    }

    #[test]
    pub fn parse_number() {
        let test_value      = r#" 1234 "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_number(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == serde_json::Value::Number(1234.into()));
        })
    }

    #[test]
    pub fn parse_value_string() {
        let test_value      = r#" "string" "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == serde_json::Value::String("string".into()));
        })
    }

    #[test]
    pub fn parse_value_number() {
        let test_value      = r#" 1234 "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == serde_json::Value::Number(1234.into()));
        })
    }

    #[test]
    pub fn parse_value_object_1() {
        use serde_json::json;

        let test_value      = r#" { } "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == json!({ }));
        })
    }

    #[test]
    pub fn parse_value_object_2() {
        use serde_json::json;

        let test_value      = r#" { "field1": 12 } "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == json!({
                "field1": 12
            }));
        })
    }

    #[test]
    pub fn parse_value_object_3() {
        use serde_json::json;

        let test_value      = r#" { "field1": 12, "field2": false } "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == json!({
                "field1": 12,
                "field2": false
            }));
        })
    }

    #[test]
    pub fn parse_value_object_4() {
        use serde_json::json;

        let test_value      = r#" { "field1": 12, "field2": { "field3": 34, "field4": 56.1 } } "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == json!({
                "field1": 12,
                "field2": {
                    "field3": 34,
                    "field4": 56.1
                }
            }));
        })
    }

    #[test]
    pub fn parse_value_array_1() {
        use serde_json::json;

        let test_value      = r#" [ ] "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == json!([ ]));
        })
    }

    #[test]
    pub fn parse_value_array_2() {
        use serde_json::json;

        let test_value      = r#" [ 1 ] "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == json!([ 1 ]));
        })
    }

    #[test]
    pub fn parse_value_array_3() {
        use serde_json::json;

        let test_value      = r#" [ 1,2,3,4 ] "#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == json!([ 1, 2, 3, 4 ]));
        })
    }

    #[test]
    pub fn parse_value_array_4() {
        use serde_json::json;

        let test_value      = r#"[ 1,2,3,4 ]"#;
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result.to_serde() == json!([ 1, 2, 3, 4 ]));
        })
    }
}

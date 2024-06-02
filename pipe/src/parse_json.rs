use crate::parser::*;

use futures::prelude::*;
use futures::future::{LocalBoxFuture};
use regex_automata::{Input};
use regex_automata::dfa::{Automaton};
use regex_automata::dfa::dense;
use once_cell::sync::{Lazy};
use itertools::*;

static NUMBER: Lazy<dense::DFA<Vec<u32>>> = Lazy::new(|| dense::DFA::new("^(-)?[0-9]+(\\.[0-9]+)?([eE]([+-])?[0-9]+)?").unwrap());
static STRING: Lazy<dense::DFA<Vec<u32>>> = Lazy::new(|| dense::DFA::new(r#"^"([^"\\]|(\\["\\/bfnrtu]))*""#).unwrap());

///
/// The tokens that make up the JSON language
///
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum JsonToken {
    Whitespace,
    Number,
    String,
    True,
    False,
    Null,
    Character(char),
}

/// Matches a string against the JSON whitespace syntax
fn match_whitespace(lookahead: &str, eof: bool) -> TokenMatchResult<JsonToken> {
    let num_whitespace = lookahead.chars()
        .take_while(|c| *c == ' ' || *c == '\n' || *c == '\r' || *c == '\t')
        .count();

    if num_whitespace == 0 {
        TokenMatchResult::LookaheadCannotMatch
    } else if num_whitespace < lookahead.len() || eof {
        TokenMatchResult::Matches(JsonToken::Whitespace, num_whitespace)
    } else {
        TokenMatchResult::LookaheadIsPrefix
    }
}

fn match_regex(dfa: &dense::DFA<Vec<u32>>, lookahead: &str, eof: bool) -> TokenMatchResult<()> {
    // Longest match in the lookahead
    let mut match_pos   = 0;
    let mut valid_pos   = 0;
    let mut state       = dfa.start_state_forward(&Input::new(lookahead)).unwrap();

    for (current_pos, byte) in lookahead.bytes().enumerate() {
        state = dfa.next_state(state, byte);

        if dfa.is_match_state(state) {
            // Found a possible match after consuming this byte
            match_pos = current_pos;
        } else if dfa.is_dead_state(state) || dfa.is_quit_state(state) {
            // Stop in dead states. Set valid_pos to 0 as this is no longer a prefix.
            valid_pos = 0;
            break;
        }

        valid_pos = current_pos + 1;
    }

    if eof && !dfa.is_dead_state(state) && !dfa.is_quit_state(state) {
        state = dfa.next_eoi_state(state);

        if dfa.is_match_state(state) {
            // Found a possible match after consuming this byte
            match_pos = lookahead.len();
        } else if dfa.is_dead_state(state) || dfa.is_quit_state(state) {
            // No longer a valid prefix
            valid_pos = 0;
        }
    }

    if valid_pos == 0 && match_pos == 0 {
        // No characters matched, so this isn't a match
        TokenMatchResult::LookaheadCannotMatch
    } else if match_pos != 0 && (eof || dfa.is_dead_state(state) || dfa.is_quit_state(state)) {
        // Finished a match
        TokenMatchResult::Matches((), lookahead
            .char_indices()
            .take_while(|(byte_index, _chr)| *byte_index < match_pos)
            .count())
    } else {
        TokenMatchResult::LookaheadIsPrefix
    }
}

/// Matches a string against the JSON number syntax
fn match_number(lookahead: &str, eof: bool) -> TokenMatchResult<JsonToken> {
    match match_regex(&*NUMBER, lookahead, eof) {
        TokenMatchResult::LookaheadIsPrefix     => TokenMatchResult::LookaheadIsPrefix,
        TokenMatchResult::LookaheadCannotMatch  => TokenMatchResult::LookaheadCannotMatch,
        TokenMatchResult::Matches(_, len)       => TokenMatchResult::Matches(JsonToken::Number, len)
    }
}

/// Matches a string against the JSON string syntax
fn match_string(lookahead: &str, eof: bool) -> TokenMatchResult<JsonToken> {
    match match_regex(&*STRING, lookahead, eof) {
        TokenMatchResult::LookaheadIsPrefix     => TokenMatchResult::LookaheadIsPrefix,
        TokenMatchResult::LookaheadCannotMatch  => TokenMatchResult::LookaheadCannotMatch,
        TokenMatchResult::Matches(_, len)       => TokenMatchResult::Matches(JsonToken::String, len)
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
/// Reads a token from the tokenizer
///
pub async fn json_read_token<TStream>(tokenizer: &mut Tokenizer<JsonToken, TStream>) -> Option<TokenMatch<JsonToken>>
where
    TStream: Stream<Item=Vec<u8>>,
{
    loop {
        // Acquire a token from the tokenizer
        let next_token = tokenizer.match_token().await?;

        // Skip over whitespace, then return the first 'sold' value
        match next_token.token? {
            JsonToken::Whitespace   => { }
            _                       => { break Some(next_token); }
        }
    }
}

///
/// Attempts to parse a JSON value starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub fn json_parse_value<'a, TStream>(parser: &'a mut Parser<TokenMatch<JsonToken>, serde_json::Value>, tokenizer: &'a mut Tokenizer<JsonToken, TStream>) -> LocalBoxFuture<'a, Result<(), ()>>
where
    TStream: Stream<Item=Vec<u8>>,
{
    async move {
        let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;

        if let Some(lookahead) = lookahead {
            // Decide which matcher to use based on the lookahead
            match lookahead.token {
                Some(JsonToken::String)         => json_parse_string(parser, tokenizer).await,
                Some(JsonToken::Number)         => json_parse_number(parser, tokenizer).await,
                Some(JsonToken::Character('{')) => json_parse_object(parser, tokenizer).await,
                Some(JsonToken::Character('[')) => json_parse_array(parser, tokenizer).await,
                Some(JsonToken::True)           => { parser.accept_token().map_err(|_| ())?.reduce(1, |_| serde_json::Value::Bool(true)).map_err(|_| ())?; Ok(()) },
                Some(JsonToken::False)          => { parser.accept_token().map_err(|_| ())?.reduce(1, |_| serde_json::Value::Bool(false)).map_err(|_| ())?; Ok(()) },
                Some(JsonToken::Null)           => { parser.accept_token().map_err(|_| ())?.reduce(1, |_| serde_json::Value::Null).map_err(|_| ())?; Ok(()) },
                _                               => Err(())
            }
        } else {
            // Error if there's no symbol
            Err(())
        }
    }.boxed_local()
}

///
/// Attempts to parse a JSON object starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub async fn json_parse_object<TStream>(parser: &mut Parser<TokenMatch<JsonToken>, serde_json::Value>, tokenizer: &mut Tokenizer<JsonToken, TStream>) -> Result<(), ()>
where
    TStream: Stream<Item=Vec<u8>>,
{
    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;
    let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(()) };

    if let Some(JsonToken::Character('{')) = lookahead.token {
        // Accept the initial '{'
        parser.accept_token().map_err(|_| ())?;

        let mut num_tokens = 1;

        loop {
            // Read two tokens ahead
            parser.ensure_lookahead(1, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;
            
            // Look to the next value to decide what to do
            let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;
            let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(()) };

            match lookahead.token {
                Some(JsonToken::Character('}')) => {
                    // '}' Finishes the object successfully
                    parser.accept_token().map_err(|_| ())?;
                    num_tokens += 1;
                    break;
                },

                Some(JsonToken::String) => {
                    // <String> : <Value>
                    parser.accept_token().map_err(|_| ())?;
                    parser.reduce(1, |string| serde_json::from_str(&string[0].token().unwrap().fragment).unwrap()).map_err(|_| ())?;
                    num_tokens += 1;

                    // ... ':'
                    parser.accept_expected_token(|token| token.token == Some(JsonToken::Character(':'))).map_err(|_| ())?;
                    num_tokens += 1;

                    json_parse_value(parser, tokenizer).await?;
                    num_tokens += 1;

                    // ',' or '}'
                    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;
                    let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(()) };

                    match lookahead.token {
                        Some(JsonToken::Character('}')) => {
                            // Ends the object
                            parser.accept_token().map_err(|_| ())?;
                            num_tokens += 1;
                            break;
                        }

                        Some(JsonToken::Character(',')) => {
                            // More fields
                            parser.accept_token().map_err(|_| ())?;
                            num_tokens += 1;
                        }

                        _ => {
                            // Unexpected value
                            return Err(());
                        }
                    }
                },

                _ => {
                    // Anything else is an error
                    return Err(());
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
                        Some(serde_json::Value::String(key))    => key,
                        _                                       => panic!(),
                    };

                    (key, value.to_node().unwrap())
                });

            serde_json::Value::Object(values.collect())
        }).map_err(|_| ())?;

        Ok(())
    } else {
        // Doesn't start with '{'
        Err(())
    }
}

///
/// Attempts to parse a JSON array starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub async fn json_parse_array<TStream>(parser: &mut Parser<TokenMatch<JsonToken>, serde_json::Value>, tokenizer: &mut Tokenizer<JsonToken, TStream>) -> Result<(), ()>
where
    TStream: Stream<Item=Vec<u8>>,
{
    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;
    let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(()) };

    if let Some(JsonToken::Character('[')) = lookahead.token {
        // Accept the initial '['
        parser.accept_token().map_err(|_| ())?;

        let mut num_tokens = 1;


        loop {
            // Look ahead to the next value
            let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;
            let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(()) };

            // ']' to finish the array, or else a JSON value
            match lookahead.token {
                Some(JsonToken::Character(']')) => {
                    // Accept the ']'
                    parser.accept_token().map_err(|_| ())?;
                    num_tokens += 1;

                    // Successfully parsed the array
                    break;
                }

                _ => {
                    // Read the next value
                    json_parse_value(parser, tokenizer).await?;
                    num_tokens += 1;

                    // Next token should be a ',' for more array or ']' for the end of the array
                    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;
                    let lookahead = if let Some(lookahead) = lookahead { lookahead } else { return Err(()) };

                    match lookahead.token {
                        Some(JsonToken::Character(',')) => {
                            // Continues the array
                            parser.accept_token().map_err(|_| ())?;
                            num_tokens += 1;
                        }

                        Some(JsonToken::Character(']')) => {
                            // Finishes the array
                            parser.accept_token().map_err(|_| ())?;
                            num_tokens += 1;
                            break;
                        }

                        _ => {
                            // Invalid value
                            return Err(());
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

            serde_json::Value::Array(values.collect())
        }).map_err(|_| ())?;

        Ok(())
    } else {
        // Not an array
        Err(())
    }
}

///
/// Attempts to parse a JSON string starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub async fn json_parse_string<TStream>(parser: &mut Parser<TokenMatch<JsonToken>, serde_json::Value>, tokenizer: &mut Tokenizer<JsonToken, TStream>) -> Result<(), ()>
where
    TStream: Stream<Item=Vec<u8>>,
{
    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;

    if let Some(lookahead) = lookahead {
        if let Some(JsonToken::String) = lookahead.token {
            // Reduce as a string
            let value = serde_json::from_str(&lookahead.fragment).map_err(|_| ())?;

            parser.accept_token().map_err(|_| ())?.reduce(1, |_| value).map_err(|_| ())?;
            Ok(())
        } else {
            // Not a string
            Err(())
        }
    } else {
        // No lookahead
        Err(())
    }
}

///
/// Attempts to parse a JSON object starting at the current location in the tokenizer, leaving the result on top of the stack in the parser
/// (or returning an error state if the value is not recognised)
///
pub async fn json_parse_number<TStream>(parser: &mut Parser<TokenMatch<JsonToken>, serde_json::Value>, tokenizer: &mut Tokenizer<JsonToken, TStream>) -> Result<(), ()>
where
    TStream: Stream<Item=Vec<u8>>,
{
    let lookahead = parser.lookahead(0, tokenizer, |tokenizer| json_read_token(tokenizer).boxed_local()).await;

    if let Some(lookahead) = lookahead {
        if let Some(JsonToken::Number) = lookahead.token {
            // Reduce as a number
            let value = serde_json::from_str(&lookahead.fragment).map_err(|_| ())?;

            parser.accept_token().map_err(|_| ())?.reduce(1, |_| value).map_err(|_| ())?;
            Ok(())
        } else {
            // Not a number
            Err(())
        }
    } else {
        // No lookahead
        Err(())
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
        let mut tokenizer   = Tokenizer::new(stream::iter(input.bytes()).ready_chunks(2));
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
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_string(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == serde_json::Value::String("string".into()));
        })
    }

    #[test]
    pub fn parse_number() {
        let test_value      = r#" 1234 "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_number(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == serde_json::Value::Number(1234.into()));
        })
    }

    #[test]
    pub fn parse_value_string() {
        let test_value      = r#" "string" "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == serde_json::Value::String("string".into()));
        })
    }

    #[test]
    pub fn parse_value_number() {
        let test_value      = r#" 1234 "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == serde_json::Value::Number(1234.into()));
        })
    }

    #[test]
    pub fn parse_value_object_1() {
        use serde_json::json;

        let test_value      = r#" { } "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == json!({ }));
        })
    }

    #[test]
    pub fn parse_value_object_2() {
        use serde_json::json;

        let test_value      = r#" { "field1": 12 } "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == json!({
                "field1": 12
            }));
        })
    }

    #[test]
    pub fn parse_value_object_3() {
        use serde_json::json;

        let test_value      = r#" { "field1": 12, "field2": false } "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == json!({
                "field1": 12,
                "field2": false
            }));
        })
    }

    #[test]
    pub fn parse_value_object_4() {
        use serde_json::json;

        let test_value      = r#" { "field1": 12, "field2": { "field3": 34, "field4": 56.1 } } "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == json!({
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
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == json!([ ]));
        })
    }

    #[test]
    pub fn parse_value_array_2() {
        use serde_json::json;

        let test_value      = r#" [ 1 ] "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == json!([ 1 ]));
        })
    }

    #[test]
    pub fn parse_value_array_3() {
        use serde_json::json;

        let test_value      = r#" [ 1,2,3,4 ] "#;
        let mut tokenizer   = Tokenizer::new(stream::iter(test_value.bytes()).ready_chunks(2));
        let mut parser      = Parser::new();
        tokenizer.with_json_matchers();

        executor::block_on(async move {
            json_parse_value(&mut parser, &mut tokenizer).await.unwrap();

            let result = parser.finish().unwrap();

            assert!(result == json!([ 1, 2, 3, 4 ]));
        })
    }
}

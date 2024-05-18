use crate::parser::*;

use regex_automata::{Input};
use regex_automata::dfa::{dense, Automaton};
use once_cell::sync::{Lazy};

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
        .filter(|c| *c == ' ' || *c == '\n' || *c == '\r' || *c == '\t')
        .count();

    if num_whitespace == 0 {
        TokenMatchResult::LookaheadCannotMatch
    } else if num_whitespace < lookahead.len() && !eof {
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

impl TokenMatcher<JsonToken> for JsonToken {
    fn try_match(&self, lookahead: &'_ str, eof: bool) -> TokenMatchResult<JsonToken> {
        use JsonToken::*;

        match self {
            Whitespace      => match_whitespace(lookahead, eof),
            Number          => match_number(lookahead, eof),
            Character(_)    => todo!(),
            String          => match_string(lookahead, eof),
            True            => todo!(),
            False           => todo!(),
            Null            => todo!(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
}

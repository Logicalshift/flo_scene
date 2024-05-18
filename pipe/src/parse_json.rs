use crate::parser::*;

use regex_automata::{Input};
use regex_automata::dfa::{dense, Automaton};
use once_cell::sync::{Lazy};

static NUMBER: Lazy<dense::DFA<Vec<u32>>> = Lazy::new(|| dense::DFA::new("(-)?[0-9]+(\\.[0-9]+)?([eE]([+-])?[0-9]+)?").unwrap());

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

/// Matches a string against the JSON number syntax
fn match_number(lookahead: &str, eof: bool) -> TokenMatchResult<JsonToken> {
    // Longest match in the lookahead
    let number          = &*NUMBER;
    let mut match_pos   = 0;
    let mut state       = number.start_state_forward(&Input::new(lookahead)).unwrap();

    for (current_pos, byte) in lookahead.bytes().enumerate() {
        state = (*NUMBER).next_state(state, byte);

        if number.is_match_state(state) {
            // Found a possible match after consuming this byte
            match_pos = current_pos;
        } else if number.is_dead_state(state) || number.is_quit_state(state) {
            // Stop in dead states
            break;
        }
    }

    if eof && !number.is_dead_state(state) && !number.is_quit_state(state) {
        state = number.next_eoi_state(state);

        if number.is_match_state(state) {
            // Found a possible match after consuming this byte
            match_pos = lookahead.len();
        }
    }

    if match_pos == 0 {
        // No characters matched, so this isn't a number
        TokenMatchResult::LookaheadCannotMatch
    } else if eof || number.is_dead_state(state) || number.is_quit_state(state) {
        // Finished a match
        TokenMatchResult::Matches(JsonToken::Number, lookahead
            .char_indices()
            .take_while(|(byte_index, _chr)| *byte_index < match_pos)
            .count())
    } else {
        TokenMatchResult::LookaheadIsPrefix
    }
}

impl TokenMatcher<JsonToken> for JsonToken {
    fn try_match(&self, lookahead: &'_ str, eof: bool) -> TokenMatchResult<JsonToken> {
        use JsonToken::*;

        match self {
            Whitespace      => match_whitespace(lookahead, eof),
            Number          => match_number(lookahead, eof),
            Character(c)    => todo!(),
            String          => todo!(),
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
    pub fn reject_not_a_number_negative() {
        let match_result = match_number("-erg", true);
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
    pub fn match_exponent_number() {
        let match_result = match_number("12e34", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 5), "{:?}", match_result);
    }

    #[test]
    pub fn match_decimal_exponent_number() {
        let match_result = match_number("12.34e56", true);
        assert!(match_result == TokenMatchResult::Matches(JsonToken::Number, 8), "{:?}", match_result);
    }
}

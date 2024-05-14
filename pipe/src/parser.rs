use futures::prelude::*;

use std::collections::{VecDeque};
use std::pin::*;
use std::sync::*;

///
/// Results of matching some lookahead aganst a token
///
pub enum TokenMatchResult<TToken> {
    /// The token matches against the start of the lookahead, by consuming the specified number of characters
    Matches(TToken, usize),

    /// The lookahead represents a prefix of this token. This can be returned even if the prefix would match this token
    /// but there could be more characters (for example when parsing an identifier, we might want to look ahead an extra
    /// character to ensure that we've got the entire length)
    LookaheadIsPrefix,

    /// The lookahead cannot match this token no matter how many new characters are added
    LookaheadCannotMatch, 
}

///
/// Trait implemented by matchers that can recognise a token
///
pub trait TokenMatcher<TToken> {
    ///
    /// Called after we've read one or more characters from the source to check if the start of the lookahead matches this token
    ///
    /// Usually the tokenizer will return the first token that matches the lookahead. If 'eof' is set then there will be no more
    /// lookahead generated.
    ///
    fn try_match(&self, lookahead: &'_ str, eof: bool) -> TokenMatchResult<TToken>;
}

///
/// A match from the tokenizer
///
#[derive(Clone)]
pub struct TokenMatch<TToken> {
    /// The token that was matched, or None if no token could be matched and the input is being discarded
    pub token: Option<TToken>,

    /// The input fragment that was matched against the token
    pub fragment: String,
}

///
/// A basic async tokenizer
///
/// This reads from a stream of u8 values, converts those to characters and matches possible tokens. This can
/// be used to build a recursive-descent parser.
///
pub struct Tokenizer<TToken, TStream> {
    /// The stream where tokens should be read from
    source_stream: Option<Pin<Box<TStream>>>,

    /// The matchers to apply to the next token
    matchers: Vec<Arc<dyn TokenMatcher<TToken>>>,

    /// Characters that have been consumed from the source stream but which have not yet fully matched a token
    lookahead_chars: String,

    /// Bytes that do not yet match a fully-formed UTF-8 character
    lookahead_bytes: VecDeque<u8>,
}

impl<TToken, TStream> Tokenizer<TToken, TStream> 
where
    TStream: Stream<Item = Vec<u8>>,
{
    ///
    /// Creates a tokenizer that will read from the specified stream
    ///
    pub fn new(stream: TStream) -> Self {
        Tokenizer { 
            source_stream:      Some(Box::pin(stream)),
            matchers:           vec![],
            lookahead_bytes:    VecDeque::new(),
            lookahead_chars:    String::new(),
        }
    }

    ///
    /// Adds a matcher to this tokenizer (returns the reference so that this can be chained to add multiple matchers)
    ///
    pub fn with_matcher(&mut self, matcher: impl 'static + TokenMatcher<TToken>) -> &mut Self {
        self.matchers.push(Arc::new(matcher));
        self
    }

    ///
    /// Removes the matchers from this tokenizer
    ///
    pub fn with_no_matchers(&mut self) -> &mut Self {
        self.matchers.clear();
        self
    }

    ///
    /// Matches the next token from the input stream
    ///
    pub async fn match_token(&mut self) -> Option<TokenMatch<TToken>> {
        todo!()
    }

    ///
    /// If the start of the lookahead matches a character, add it to the lookahead. Returns false if no character could be added.
    ///
    fn read_lookahead_character(&mut self) -> bool {
        // When we skip a character, this is the invalid character we push
        const INVALID_CHAR: char = '\0';

        if self.lookahead_bytes.is_empty() {
            // No lookahead
            false
        } else if self.lookahead_bytes[0] <= 127 {
            // First character is ASCII
            self.lookahead_chars.push(unsafe { char::from_u32_unchecked(self.lookahead_bytes[0] as u32) });
            self.lookahead_bytes.pop_front();
            true
        } else {
            // Decode the first character
            let first = self.lookahead_bytes[0];
            let (num_extra, first) = if first&0b1110_0000 == 0b1100_0000 {
                (1, first&0b0001_1111)
            } else if first&0b1111_0000 == 0b1110_0000 {
                (2, first&0b0000_1111)
            } else if first&0b1111_1000 == 0b1111_0000 {
                (3, first&0b0000_0111)
            } else {
                // Not a valid start character (push an invalid character and remove from the lookahead)
                self.lookahead_chars.push(INVALID_CHAR);
                self.lookahead_bytes.pop_front();
                return true;
            };

            if self.lookahead_bytes.len() < 1+num_extra {
                // Needs to be a certain number of characters in the lookahead to match this character
                false
            } else {
                // Read 'num_extra' bytes from the lookahead; start by removing the first character
                self.lookahead_bytes.pop_front();

                for p in 0..num_extra {
                    if self.lookahead_bytes[p]&0b1100_0000 != 0b1000_0000 {
                        // All the following bytes must have this form (just skip the first character if they don't)
                        self.lookahead_chars.push(INVALID_CHAR);
                        return true;
                    }
                }

                // Should be able to make a valid unicode character from this
                let u32_chr = if num_extra == 1 {
                    ((first as u32)<<6) | ((self.lookahead_bytes[0]&0b0011_1111) as u32)
                } else if num_extra == 2 {
                    ((first as u32)<<12) | (((self.lookahead_bytes[0]&0b0011_1111) as u32)<<6) | ((self.lookahead_bytes[1]&0b0011_1111) as u32)
                } else if num_extra == 3 {
                    ((first as u32)<<18) | (((self.lookahead_bytes[0]&0b0011_1111) as u32)<<12) | (((self.lookahead_bytes[1]&0b0011_1111) as u32)<<6) | ((self.lookahead_bytes[2]&0b0011_1111) as u32)
                } else {
                    unreachable!()
                };

                // Add the character
                self.lookahead_chars.push(unsafe { char::from_u32_unchecked(u32_chr) });

                // Remove the bytes
                for _ in 0..num_extra {
                    self.lookahead_bytes.pop_front();
                }

                true
            }
        }
    }

    ///
    /// Try to read more characters from the lookahead
    ///
    /// Returns false if the stream if exhausted and no character could be matched, or true if an extra character has been appended to the lookahead characters
    ///
    async fn read_next_char(&mut self) -> bool {
        loop {
            // Accept characters from the lookahead if we can

            // Read the next batch of characters from the stream
            if let Some(stream) = &mut self.source_stream {
                if let Some(next_bytes) = stream.next().await {
                    self.lookahead_bytes.extend(next_bytes);
                } else {
                    // The stream is closed
                    self.source_stream = None;
                }
            } else {
                // The lookahead doesn't contain a valid character or anything to discard and the stream is empty
                return false;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn decode_utf8_1() {
        let mut state = Tokenizer::<(), stream::Empty<Vec<u8>>> {
            source_stream:      None,
            matchers:           vec![],
            lookahead_chars:    String::new(),
            lookahead_bytes:    vec![0x24].into_iter().collect(),
        };

        assert!(state.read_lookahead_character());
        assert!(state.lookahead_chars == "$");
    }

    #[test]
    fn decode_utf8_2() {
        let mut state = Tokenizer::<(), stream::Empty<Vec<u8>>> {
            source_stream:      None,
            matchers:           vec![],
            lookahead_chars:    String::new(),
            lookahead_bytes:    vec![0xc2, 0xa3].into_iter().collect(),
        };

        assert!(state.read_lookahead_character());
        assert!(state.lookahead_chars == "£");
        assert!(state.lookahead_bytes.is_empty());
    }

    #[test]
    fn decode_utf8_3() {
        let mut state = Tokenizer::<(), stream::Empty<Vec<u8>>> {
            source_stream:      None,
            matchers:           vec![],
            lookahead_chars:    String::new(),
            lookahead_bytes:    vec![0xe0, 0xa4, 0xb9].into_iter().collect(),
        };

        assert!(state.read_lookahead_character());
        assert!(state.lookahead_chars == "ह");
        assert!(state.lookahead_bytes.is_empty());
    }

    #[test]
    fn decode_utf8_4() {
        let mut state = Tokenizer::<(), stream::Empty<Vec<u8>>> {
            source_stream:      None,
            matchers:           vec![],
            lookahead_chars:    String::new(),
            lookahead_bytes:    vec![0xf0, 0x90, 0x8d, 0x88].into_iter().collect(),
        };

        assert!(state.read_lookahead_character());
        assert!(state.lookahead_chars == "𐍈", "{:?} {:x}", state.lookahead_chars, state.lookahead_chars.chars().next().unwrap() as u32);
        assert!(state.lookahead_bytes.is_empty());
    }
}
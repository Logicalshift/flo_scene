use super::program::*;
use super::location::*;
use super::parse_error::*;
use super::pushback_stream::*;

use flo_stream::*;

use futures::prelude::*;
use futures::task::{Poll, Context};

use std::sync::*;
use std::pin::*;

///
/// A parser result
///
#[derive(Clone, PartialEq, Debug)]
pub struct ParserResult<TResult> {
    /// The parsed value
    pub value: TResult,

    /// The location where this result was generated from
    pub location: TalkLocation,

    /// The text that was matched for this result
    pub matched: Arc<String>,
}

/// True if the specified character is a whitespace character
#[inline]
fn is_whitespace(c: char) -> bool {
    c.is_whitespace()
}

impl<TStream> PushBackStream<TStream>
where
    TStream: Unpin + Send + Stream<Item=char>
{
    ///
    /// Consumes as much whitespace as possible
    ///
    async fn consume_whitespace(&mut self) {
        // Read characters until we receive a non-whitespace character, then push it back
        while let Some(c) = self.next().await {
            if !is_whitespace(c) {
                self.pushback(c);
                break;
            }
        }
    }

    ///
    /// Consumes a comment, if one exists at the present location, returning as an empty parser result
    ///
    async fn consume_comment(&mut self) -> Option<Result<ParserResult<()>, ParserResult<TalkParseError>>> {
        // In Smalltalk, comments start with a double-quote character '"'
        if self.peek().await != Some('"') { return None; }

        // Remember where the comment starts
        let comment_start   = self.location();
        let mut matched     = String::new();

        // Consume the first '"'
        let first_quote = self.next().await;
        debug_assert!(first_quote == Some('"'));
        matched.push(first_quote.unwrap());

        // Read until the closing '"' (or the end of the stream)
        while let Some(chr) = self.next().await {
            matched.push(chr);

            if chr == '"' {
                // End of comment
                return Some(Ok(ParserResult { value: (), location: comment_start.to(self.location()), matched: Arc::new(matched) }));
            }
        }

        Some(Err(ParserResult { value: TalkParseError::UnclosedDoubleQuoteComment, location: comment_start.to(self.location()), matched: Arc::new(matched) }))
    }

    ///
    /// Matches and returns the next expression on this stream
    ///
    async fn match_expression(&mut self) -> Option<Result<ParserResult<TalkExpression>, ParserResult<TalkParseError>>> {
        // Eat up as much whitespace as possible
        self.consume_whitespace().await;

        None
    }
}

///
/// Parses a flotalk expression stream
///
pub fn parse_flotalk_expression<'a>(input_stream: impl 'a + Unpin + Send + Stream<Item=char>) -> impl 'a + Send + Stream<Item=Result<ParserResult<TalkExpression>, ParserResult<TalkParseError>>> {
    let input_stream = PushBackStream::new(input_stream);

    // Use a generator stream to output the values
    generator_stream(move |yield_value| async move {
        let mut input_stream = input_stream;

        // Match as many expressions as possible
        while let Some(expression) = input_stream.match_expression().await {
            yield_value(expression).await;
        }
    })
}

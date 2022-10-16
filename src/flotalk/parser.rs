use super::program::*;
use super::location::*;
use super::parse_error::*;
use super::pushback_stream::*;

use flo_stream::*;

use futures::prelude::*;
use futures::stream;
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

impl<TStream> PushBackStream<TStream>
where
    TStream: Unpin + Send + Stream<Item=char>
{
    ///
    /// Matches and returns the next expression on this stream
    ///
    async fn match_expression(&mut self) -> Option<Result<ParserResult<TalkExpression>, ParserResult<TalkParseError>>> {
        None
    }
}

///
/// Parses a flotalk expression stream
///
pub fn parse_flotalk_expression<'a>(input_stream: impl 'a + Unpin + Send + Stream<Item=char>) -> impl 'a + Send + Stream<Item=Result<ParserResult<TalkExpression>, ParserResult<TalkParseError>>> {
    let input_stream = PushBackStream::new(input_stream);

    generator_stream(move |yield_value| async move {
        let mut input_stream = input_stream;

        while let Some(expression) = input_stream.match_expression().await {
            yield_value(expression).await;
        }
    })
}

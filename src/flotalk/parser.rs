use super::program::*;
use super::location::*;
use super::parse_error::*;

use flo_stream::*;

use futures::prelude::*;
use futures::stream;

use std::sync::*;

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

///
/// Parses a flotalk stream
///
pub fn parse_flotalk<'a>(input_stream: impl 'a + Send + Stream<Item=char>) -> impl 'a + Send + Stream<Item=Result<ParserResult<TalkExpression>, ParserResult<TalkParseError>>> {
    stream::empty()
}

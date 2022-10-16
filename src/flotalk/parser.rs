use super::program::*;
use super::location::*;
use super::parse_error::*;

use flo_stream::*;

use futures::prelude::*;
use futures::stream;

use std::sync::*;

///
/// Parses a flotalk stream
///
pub fn parse_flotalk<'a>(input_stream: impl 'a + Send + Stream<Item=char>) -> impl 'a + Send + Stream<Item=Result<(TalkLocation, TalkExpression), (TalkLocation, TalkParseError)>> {
    stream::empty()
}

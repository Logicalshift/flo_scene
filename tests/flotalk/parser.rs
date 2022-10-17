use flo_scene::flotalk::*;

use futures::prelude::*;
use futures::stream;
use futures::executor;

use std::sync::*;

#[test]
fn character_literal() {
    let test_source     = "$a";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Literal(TalkLiteral::Character('a')));
}

#[test]
fn string_literal() {
    let test_source     = "'string'";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Literal(TalkLiteral::String(Arc::new("string".to_string()))));
}

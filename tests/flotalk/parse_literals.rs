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

#[test]
fn string_literal_with_quote() {
    let test_source     = "'string''quote'";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Literal(TalkLiteral::String(Arc::new("string'quote".to_string()))));
}

#[test]
fn symbol_literal() {
    let test_source     = "#'symbol'";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Literal(TalkLiteral::Symbol(Arc::new("symbol".to_string()))));
}

#[test]
fn selector_literal() {
    let test_source     = "#selector";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Literal(TalkLiteral::Selector(Arc::new("selector".to_string()))));
}

#[test]
fn selector_literal_keyword() {
    let test_source     = "#selector:";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Literal(TalkLiteral::Selector(Arc::new("selector:".to_string()))));
}

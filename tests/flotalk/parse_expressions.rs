use flo_scene::flotalk::*;

use futures::prelude::*;
use futures::stream;
use futures::executor;

use std::sync::*;

#[test]
fn identifier_expression() {
    let test_source     = "identifier";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Identifier(Arc::new("identifier".to_string())));
}

#[test]
fn identifier_expression_with_whitespace() {
    let test_source     = "   \nidentifier";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Identifier(Arc::new("identifier".to_string())));
}

#[test]
fn identifier_expression_with_comment() {
    let test_source     = "\"Some comment\" identifier";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Identifier(Arc::new("identifier".to_string())));
}

#[test]
fn bracketed_identifier_expression() {
    let test_source     = "( identifier )";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Identifier(Arc::new("identifier".to_string())));
}

#[test]
fn empty_expression() {
    let test_source     = ".";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Empty);
}

#[test]
fn variable_declaration() {
    let test_source     = "| a b c foo |";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::VariableDeclaration(vec![
        Arc::new("a".to_string()),
        Arc::new("b".to_string()),
        Arc::new("c".to_string()),
        Arc::new("foo".to_string()),
    ]));
}

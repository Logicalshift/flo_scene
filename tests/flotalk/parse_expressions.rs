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

#[test]
fn empty_block() {
    let test_source     = "[ ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Block(vec![], vec![]));
}

#[test]
fn identifier_block() {
    let test_source     = "[ identifier ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Block(vec![], vec![TalkExpression::Identifier(Arc::new("identifier".to_string()))]));
}

#[test]
fn arguments_block() {
    let test_source     = "[ :foo :bar | identifier ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Block(vec![Arc::new("foo".to_string()), Arc::new("bar".to_string())], vec![TalkExpression::Identifier(Arc::new("identifier".to_string()))]));
}

#[test]
fn assignment() {
    let test_source     = "foo := 1";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::Assignment(Arc::new("foo".to_string()), Box::new(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string()))))));
}

#[test]
fn unary_message() {
    let test_source     = "foo unaryMessage";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::SendMessages(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
        vec![TalkArgument { name: Arc::new("unaryMessage".to_string()), value: None }]));
}

#[test]
fn binary_message() {
    let test_source     = "foo + 1";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::SendMessages(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
        vec![TalkArgument { name: Arc::new("+".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))) }]));
}

#[test]
fn keyword_message() {
    let test_source     = "foo someParameter: 1";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::SendMessages(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
        vec![TalkArgument { name: Arc::new("someParameter:".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))) }]));
}

#[test]
fn keyword_message_extra_parameter() {
    let test_source     = "foo someParameter: 1 withValue: 2";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::SendMessages(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))),
        vec![
            TalkArgument { name: Arc::new("someParameter:".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))) }, 
            TalkArgument { name: Arc::new("withValue:".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("2".to_string())))) }
        ]));
}

#[test]
fn keyword_message_with_binary() {
    let test_source     = "foo someParameter: 1 + 2 withValue: 3 + 4";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr == TalkExpression::SendMessages(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
        vec![
            TalkArgument { name: Arc::new("someParameter:".to_string()), value: Some(TalkExpression::SendMessages(Box::new(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))), 
                vec![TalkArgument { name: Arc::new("+".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("2".to_string())))) }])) }, 
            TalkArgument { name: Arc::new("withValue:".to_string()), value: Some(TalkExpression::SendMessages(Box::new(TalkExpression::Literal(TalkLiteral::Number(Arc::new("3".to_string())))), 
                vec![TalkArgument { name: Arc::new("+".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("4".to_string())))) }])) }
        ]));
}

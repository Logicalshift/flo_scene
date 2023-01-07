use flo_talk::*;

use futures::prelude::*;
use futures::stream;
use futures::executor;

use std::sync::*;

#[test]
fn postcard() {
    // The expressions from 'Smalltalk on a postcard' (should parse without error)
    let test_source = "| y |
        true & false not & (nil isNil) ifFalse: [self halt].
        y := self size + super size.
        #($a #a 'a' 1 1.0)
            do: [ :each |
                Transcript show: (each class name);
                           show: ' '].
        ^x < y";
    let test_source     = stream::iter(test_source.chars());
    let mut parser      = parse_flotalk_expression(test_source);
    executor::block_on(async { 
        parser.next().await.unwrap().unwrap();   // | y |
        parser.next().await.unwrap().unwrap();   // true & false not & (nil isNil) ifFalse: [self halt]
        parser.next().await.unwrap().unwrap();   // y := self size + super size.
        parser.next().await.unwrap().unwrap();   // #($a #a 'a' 1 1.0) ...
        parser.next().await.unwrap().unwrap();   // ^x < y

        assert!(parser.next().await.is_none());
    });
}

#[test]
fn postcard_binary_expression() {
    let test_source     = "true & false not & (nil isNil) ifFalse: [self halt]";
    let test_source     = stream::iter(test_source.chars());
    executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });
}

#[test]
fn identifier_expression() {
    let test_source     = "identifier";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Identifier(Arc::new("identifier".to_string())));
}

#[test]
fn identifier_expression_with_whitespace() {
    let test_source     = "   \nidentifier";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Identifier(Arc::new("identifier".to_string())));
}

#[test]
fn identifier_expression_with_comment() {
    let test_source     = "\"Some comment\" identifier";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Identifier(Arc::new("identifier".to_string())));
}

#[test]
fn bracketed_identifier_expression() {
    let test_source     = "( identifier )";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Identifier(Arc::new("identifier".to_string())));
}

#[test]
fn empty_expression() {
    let test_source     = ".";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Empty);
}

#[test]
fn variable_declaration() {
    let test_source     = "| a b c foo |";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::VariableDeclaration(vec![
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
    assert!(expr.strip() == TalkExpression::Block(vec![], vec![]));
}

#[test]
fn identifier_block() {
    let test_source     = "[ identifier ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Block(vec![], vec![TalkExpression::Identifier(Arc::new("identifier".to_string()))]));
}

#[test]
fn two_identifiers_block_1() {
    let test_source     = "[ identifier . identifier ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Block(vec![], vec![
        TalkExpression::Identifier(Arc::new("identifier".to_string())),
        TalkExpression::Empty,
        TalkExpression::Identifier(Arc::new("identifier".to_string())),
    ]));
}

#[test]
fn two_identifiers_block_2() {
    let test_source     = "[ identifier. identifier ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Block(vec![], vec![
        TalkExpression::Identifier(Arc::new("identifier".to_string())),
        TalkExpression::Empty,
        TalkExpression::Identifier(Arc::new("identifier".to_string())),
    ]));
}

#[test]
fn two_numbers_block_1() {
    let test_source     = "[ 1 . 2 ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    let expr            = expr.strip();
    assert!(expr == TalkExpression::Block(vec![], vec![
        TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string()))),
        TalkExpression::Empty,
        TalkExpression::Literal(TalkLiteral::Number(Arc::new("2".to_string()))),
    ]));
}

#[test]
fn two_numbers_block_2() {
    let test_source     = "[ 1. 2 ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    let expr            = expr.strip();
    assert!(expr == TalkExpression::Block(vec![], vec![
        TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string()))),
        TalkExpression::Empty,
        TalkExpression::Literal(TalkLiteral::Number(Arc::new("2".to_string()))),
    ]));
}

#[test]
fn arguments_block() {
    let test_source     = "[ :foo :bar | identifier ]";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Block(vec![Arc::new("foo".to_string()), Arc::new("bar".to_string())], vec![TalkExpression::Identifier(Arc::new("identifier".to_string()))]));
}

#[test]
fn assignment() {
    let test_source     = "foo := 1";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::Assignment(Arc::new("foo".to_string()), Box::new(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string()))))));
}

#[test]
fn unary_message() {
    let test_source     = "foo unaryMessage";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::SendMessage(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
        vec![TalkArgument { name: Arc::new("unaryMessage".to_string()), value: None }]));
}

#[test]
fn binary_message() {
    let test_source     = "foo + 1";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::SendMessage(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
        vec![TalkArgument { name: Arc::new("+".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))) }]));
}

#[test]
fn keyword_message() {
    let test_source     = "foo someParameter: 1";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::SendMessage(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
        vec![TalkArgument { name: Arc::new("someParameter:".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))) }]));
}

#[test]
fn keyword_message_extra_parameter() {
    let test_source     = "foo someParameter: 1 withValue: 2";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::SendMessage(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))),
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
    assert!(expr.strip() == TalkExpression::SendMessage(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
        vec![
            TalkArgument { name: Arc::new("someParameter:".to_string()), value: Some(TalkExpression::SendMessage(Box::new(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))), 
                vec![TalkArgument { name: Arc::new("+".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("2".to_string())))) }])) }, 
            TalkArgument { name: Arc::new("withValue:".to_string()), value: Some(TalkExpression::SendMessage(Box::new(TalkExpression::Literal(TalkLiteral::Number(Arc::new("3".to_string())))), 
                vec![TalkArgument { name: Arc::new("+".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("4".to_string())))) }])) }
        ]));
}

#[test]
fn unary_then_keyword_message() {
    let test_source     = "foo unaryMessage keyword: 1";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::SendMessage(
            Box::new(TalkExpression::SendMessage(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
                vec![TalkArgument { name: Arc::new("unaryMessage".to_string()), value: None }])), 
        vec![TalkArgument { name: Arc::new("keyword:".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))) }]));
}

#[test]
fn cascaded_messages() {
    let test_source     = "foo unaryMessage keyword: 1; cascade: 2; anotherUnary cascade: 3";
    let test_source     = stream::iter(test_source.chars());
    let parse_result    = executor::block_on(async { parse_flotalk_expression(test_source).next().await.unwrap().unwrap() });

    let expr            = parse_result.value;
    assert!(expr.strip() == TalkExpression::CascadeFrom(Box::new(TalkExpression::Identifier(Arc::new("foo".to_string()))), 
            vec![
                TalkExpression::SendMessage(Box::new(TalkExpression::SendMessage(Box::new(TalkExpression::CascadePrimaryResult), vec![TalkArgument { name: Arc::new("unaryMessage".to_string()), value: None }])), vec![TalkArgument { name: Arc::new("keyword:".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("1".to_string())))) }]), 
                TalkExpression::SendMessage(Box::new(TalkExpression::CascadePrimaryResult), vec![TalkArgument { name: Arc::new("cascade:".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("2".to_string())))) }]), 
                TalkExpression::SendMessage(Box::new(TalkExpression::SendMessage(Box::new(TalkExpression::CascadePrimaryResult), vec![TalkArgument { name: Arc::new("anotherUnary".to_string()), value: None }])), vec![TalkArgument { name: Arc::new("cascade:".to_string()), value: Some(TalkExpression::Literal(TalkLiteral::Number(Arc::new("3".to_string())))) }])
            ])

        );
}


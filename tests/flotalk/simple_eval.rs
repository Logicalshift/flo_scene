use flo_scene::flotalk::*;

use futures::prelude::*;
use futures::executor;

use std::sync::*;

#[test]
fn evaluate_number() {
    let test_source     = "42";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn add_numbers() {
    let test_source     = "38 + 4";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn equal_numbers() {
    let test_source     = "(38 + 4) == 42";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Bool(true));
    });
}

#[test]
fn divide_numbers() {
    let test_source     = "1021.2 // 24.2";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn and_success() {
    let test_source     = "(1 < 2) and: [ (3 < 4) ]";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Bool(true));
    });
}

#[test]
fn and_failure_rhs() {
    let test_source     = "(1 < 2) and: [ (3 > 4) ]";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Bool(false));
    });
}

#[test]
fn if_true_if_false_when_true() {
    let test_source     = "(1 < 2) ifTrue: [ 42 ] ifFalse: [ 43 ]";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn if_true_if_false_when_false() {
    let test_source     = "(1 > 2) ifTrue: [ 43 ] ifFalse: [ 42 ]";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn if_false_if_true_when_true() {
    let test_source     = "(1 < 2) ifFalse: [ 43 ] ifTrue: [ 42 ]";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn and_failure_lhs() {
    let test_source     = "(1 > 2) and: [ (3 < 4) ]";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Bool(false));
    });
}

#[test]
fn retrieve_argument() {
    let test_source     = "[:x | x] value: 42";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn retrieve_root_value() {
    let test_source     = "x";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("x".into(), TalkValue::Int(42))], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn call_block() {
    let test_source     = "[ 42 ] value";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn call_block_with_arguments() {
    let test_source     = "[ :x | ^x ] value: 42";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn perform_message() {
    let test_source     = "42 perform: #yourself";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn perform_message_with() {
    let test_source     = "41 perform: #+ with: 1";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn responds_to_responds_to() {
    let test_source     = "42 respondsTo: #respondsTo:";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Bool(true));
    });
}

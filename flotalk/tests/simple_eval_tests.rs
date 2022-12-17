use flo_talk::*;

use futures::prelude::*;
use futures::executor;

use std::sync::*;

#[test]
fn evaluate_number() {
    let test_source = TalkScript::from("42");
    let runtime     = TalkRuntime::empty();

    executor::block_on(async { 
        let result = runtime.run(test_source).await;

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
    let test_source     = TalkScript::from("x");
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        runtime.set_root_symbol_value("x", 42).await;
        let result = runtime.run(test_source).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn assign_local_variable() {
    // As we only evaluate a single expression, we need to use a block expresion here
    let test_source     = "[ | y | y := 8 . x + y ] value + y";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("x".into(), TalkValue::Int(21)), ("y".into(), TalkValue::Int(13))], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn overwrite_root_variable() {
    // As we only evaluate a single expression, we need to use a block expresion here
    let test_source     = "[ y := 10 . x + y ] value + y";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        // The expression expands to 'x + y + y', but we change the value of 'y' in the block so we should see '22 + 10 + 10' and not '22 + 13 + 13' or '22 + 10 + 13'
        let result          = runtime.run_with_symbols(|_| vec![("x".into(), TalkValue::Int(22)), ("y".into(), TalkValue::Int(13))], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn overwrite_closure_variable() {
    let test_source = TalkScript::from("[ | y | y := 10 . [ y := 8 . y ] value + y ] value + y");
    let runtime     = TalkRuntime::empty();

    executor::block_on(async { 
        runtime.set_root_symbol_value("x", 22).await;
        runtime.set_root_symbol_value("y", 26).await;

        // Should overwrite the 'inner' y here to give us 8 + 8 + 26 (as the outer 'y' has a value of 26)
        let result = runtime.run(test_source).await;

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

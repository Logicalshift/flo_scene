use flo_scene::flotalk::*;

use futures::prelude::*;
use futures::executor;

use std::sync::*;

#[test]
fn evaluate_number() {
    let test_source     = "42";
    let runtime         = TalkRuntime::empty();
    let root_values     = Arc::new(Mutex::new(TalkValueStore::default()));

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn call_block() {
    let test_source     = "[ 42 ] value";
    let runtime         = TalkRuntime::empty();
    let root_values     = Arc::new(Mutex::new(TalkValueStore::default()));

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

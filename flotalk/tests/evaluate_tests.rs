use flo_talk::*;

use futures::executor;

#[test]
fn evaluate_statement() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        let msg     = runtime.run(TalkScript::from("Evaluate statement: '42'")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn read_defined_value() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        runtime.run(TalkContinuation::from(42).define_as("test")).await;
        let msg     = runtime.run(TalkScript::from("Evaluate statement: 'test'")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn evaluate_block() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        let msg     = runtime.run(TalkScript::from("
            | block |
            block := Evaluate createBlock: '42'.
            block value
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn block_read_defined_value() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        runtime.run(TalkContinuation::from(42).define_as("test")).await;
        let msg     = runtime.run(TalkScript::from("
            | block |
            block := Evaluate createBlock: 'test'.
            block value
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

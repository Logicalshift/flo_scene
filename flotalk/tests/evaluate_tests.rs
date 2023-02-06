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

#[test]
fn instance_read_defined_value() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        runtime.run(TalkContinuation::from(42).define_as("test")).await;
        let msg     = runtime.run(TalkScript::from("
            | evaluator |

            evaluator := Evaluate new.

            evaluator statement: 'test'
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn instance_redefine_value_1() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        runtime.run(TalkContinuation::from(20).define_as("test")).await;
        let msg     = runtime.run(TalkScript::from("
            | evaluator |

            evaluator := Evaluate new.
            evaluator define: #'test' as: 22.

            (evaluator statement: 'test') + test
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn instance_redefine_value_2() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        runtime.run(TalkContinuation::from(0).define_as("test")).await;
        let msg     = runtime.run(TalkScript::from("
            | evaluator |

            evaluator := Evaluate new.
            evaluator define: #'test' as: 22.

            test := 20.

            (evaluator statement: 'test') + test
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn instance_redefine_value_3() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        let msg     = runtime.run(TalkScript::from("
            | evaluator test |

            evaluator := Evaluate new.
            evaluator define: #'test' as: 22.

            test := 20.

            (evaluator statement: 'test') + test
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn instance_redefine_value_from_empty() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        runtime.run(TalkContinuation::from(20).define_as("test")).await;
        let msg     = runtime.run(TalkScript::from("
            | evaluator |

            evaluator := Evaluate newEmpty.
            evaluator define: #'test' as: 22.

            (evaluator statement: 'test') + test
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn instance_empty_state() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;
        runtime.run(TalkContinuation::from(20).define_as("test")).await;
        let msg     = runtime.run(TalkScript::from("
            | evaluator |

            evaluator := Evaluate newEmpty.

            evaluator statement: 'test'
        ")).await;

        println!("{:?}", msg);
        assert!(msg.is_error());
    })
}

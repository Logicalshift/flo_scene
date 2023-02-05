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

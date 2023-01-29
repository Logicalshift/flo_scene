use flo_talk::*;

use futures::executor;

#[test]
fn call_simple_function_block() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Send the 'value:' message to a function block
        let result = runtime.run(
            talk_fn_block(|val: i32| val + 24)
            .and_then_soon_if_ok(|fn_block, talk_context| {
                fn_block.send_message_in_context(TalkMessage::with_arguments(vec![("value:", 18)]), talk_context)
            })
        ).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

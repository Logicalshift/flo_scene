use flo_talk::*;

use futures::executor;

#[test]
fn inverted_subclass() {
    // Just subclass 'Inverted' and make sure it returns a result
    let test_source = "Inverted subclass";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should not generate an error
        assert!(match &*result {
            TalkValue::Error(_) => false,
            _                   => true
        });
    });
}

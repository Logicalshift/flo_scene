use flo_talk::*;

use futures::executor;

#[test]
fn async_value() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a background task and retrieve its result
        let result = runtime.run(TalkScript::from("
            | later |

            later := Later async: [ 42 ].
            later value
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn error_if_sender_dropped() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a background task and retrieve its result
        let result = runtime.run(TalkScript::from("
            | later laterSender |

            later       := Later new.
            laterSender := later sender.

            laterSender := 0 .
            later value
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Error(TalkError::NoResult));
    });
}

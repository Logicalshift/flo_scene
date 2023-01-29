use flo_talk::*;

use futures::executor;

#[test]
fn string_do_count_characters() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Count the number of characters in a string by calling the 'do' iterator
        let result = runtime.run(TalkScript::from("
            | count |

            count := 0

            'Test string' do: [ :char | count := count + 1. ].
            count
            ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(11));
    });
}

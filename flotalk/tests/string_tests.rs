use flo_talk::*;

use futures::executor;

use std::sync::*;

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

#[test]
fn string_print_string() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Write out a string as as tring
        let result = runtime.run(TalkScript::from("
            'Test string' printString
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::String(Arc::new("Test string".into())));
    })
}

#[test]
fn string_print_character() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Write out a string as as tring
        let result = runtime.run(TalkScript::from("
            $a printString
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::String(Arc::new("a".into())));
    })
}

#[test]
fn string_print_int() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Write out a string as as tring
        let result = runtime.run(TalkScript::from("
            42 printString
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::String(Arc::new("42".into())));
    })
}

#[test]
fn string_print_array() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Write out a string as as tring
        let result = runtime.run(TalkScript::from("
            #(1 2 3) printString
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::String(Arc::new("#(1 2 3)".into())));
    })
}

#[test]
fn string_print_messages() {
    executor::block_on(async {
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Write out a string as as tring
        let result = runtime.run(TalkScript::from("
            (#foo:bar: withArguments: #(1 2)) printString
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::String(Arc::new("##foo: 1 bar: 2".into())));
    })
}

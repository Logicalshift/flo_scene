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
        println!("{:?}", result);
        assert!(match &*result {
            TalkValue::Error(_) => false,
            _                   => true
        });
    });
}

#[test]
fn send_inverted_mesasge() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted invertedInstance object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #setValInverted: withAction: [ :newVal :sender :self | val := newVal ].

        val                 := 0.
        invertedInstance    := TestInverted new.
        object              := Object new.

        invertedInstance receiveFrom: object.
        object setValInverted: 42.

        val
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 42 via an inverted message
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

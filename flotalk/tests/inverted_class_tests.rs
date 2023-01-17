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
fn send_inverted_message_directly() {
    // Create an inverted subclass and send the 'internal' version of the inverted message
    let test_source = "
        | TestInverted invertedInstance object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #setValInverted: withAction: [ :newVal :sender :self | val := newVal ].

        val                 := 0.
        invertedInstance    := TestInverted new.
        object              := Object new.

        invertedInstance setValInverted: 42 invertedFrom: object.

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

#[test]
fn send_inverted_message_with_no_receiver() {
    // Create an inverted subclass and send the message with no receiver attached (should produce no result)
    let test_source = "
        | TestInverted invertedInstance object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #setValInverted: withAction: [ :newVal :sender :self | val := newVal ].

        val                 := 0.
        invertedInstance    := TestInverted new.
        object              := Object new.

        object setValInverted: 42
    ";

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
fn send_inverted_message() {
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

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
            TalkValue::Nil  => true,
            _               => false,
        });
    });
}

#[test]
fn all_unreceived() {
    let test_source = "all unreceived";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 42 via an inverted message
        println!("{:?}", result);
        assert!(!result.is_error());
    });
}

#[test]
fn object_unreceived() {
    let test_source = "
        | object |
        object := Object new.
        object unreceived
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 42 via an inverted message
        println!("{:?}", result);
        assert!(!result.is_error());
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

#[test]
fn send_inverted_message_result() {
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
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 42 via an inverted message
        println!("{:?}", result);
        assert!(*result == TalkValue::Nil);
    });
}

#[test]
fn send_inverted_message_in_block() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted invertedInstance object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #setValInverted: withAction: [ :newVal :sender :self | val := newVal ].

        val                 := 0.
        invertedInstance    := TestInverted new.
        object              := Object new.

        invertedInstance receiveFrom: object.
        [ object setValInverted: 42 ] value.

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
fn send_inverted_message_to_all() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted invertedInstance object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #setValInverted: withAction: [ :newVal :sender :self | val := newVal ].

        val                 := 0.
        invertedInstance    := TestInverted new.
        object              := Object new.

        invertedInstance receiveFrom: all.
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

#[test]
fn send_inverted_message_to_local_context() {
    // Create an inverted subclass and send a message to it from a 'normal' object, using a local context (then send another message outside the context to demonstrate that the context resets properly)
    let test_source = "
        | TestInverted invertedInstance object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #setValInverted: withAction: [ :newVal :sender :self | val := newVal ].

        val                 := 0.
        invertedInstance    := TestInverted new.
        object              := Object new.

        object setValInverted: 41.
        invertedInstance with: [ object setValInverted: 42 ].
        object setValInverted: 43.

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
fn send_inverted_message_to_several_targets() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted invertedInstance1 invertedInstance2 object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := val + 1 ].

        val                 := 0.
        invertedInstance1   := TestInverted new.
        invertedInstance2   := TestInverted new.
        object              := Object new.

        invertedInstance1 receiveFrom: object.
        invertedInstance2 receiveFrom: object.
        object invertedMessage.

        val
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 2 as both messages should get processed
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(2));
    });
}

#[test]
fn unreceived_filters_handled_messages() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted invertedInstance1 invertedInstance2 object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := val + 1 ].

        val                 := 0.
        invertedInstance1   := TestInverted new.
        invertedInstance2   := TestInverted new.
        object              := Object new.

        invertedInstance1 receiveFrom: object unreceived.
        invertedInstance2 receiveFrom: object.
        object invertedMessage.

        val
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 1 as the 'unreceived' version of the message is processed after the 'received' version
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(1));
    });
}

#[test]
fn unreceived_processes_earliest_messages() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted invertedInstance1 invertedInstance2 object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := val + 1 ].

        val                 := 0.
        invertedInstance1   := TestInverted new.
        invertedInstance2   := TestInverted new.
        object              := Object new.

        invertedInstance1 receiveFrom: object.
        invertedInstance2 receiveFrom: object unreceived.
        object invertedMessage.

        val
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 2 as both messages should get processed as the 'unreceived' version of the message should be processed first
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(2));
    });
}

#[test]
fn unreceived_filters_handled_messages_all() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted invertedInstance1 invertedInstance2 object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := val + 1 ].

        val                 := 0.
        invertedInstance1   := TestInverted new.
        invertedInstance2   := TestInverted new.
        object              := Object new.

        invertedInstance1 receiveFrom: all unreceived.
        invertedInstance2 receiveFrom: all.
        object invertedMessage.

        val
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 1 as the 'unreceived' version of the message is processed after the 'received' version
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(1));
    });
}

#[test]
fn unreceived_processes_earliest_messages_all() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted invertedInstance1 invertedInstance2 object val |

        TestInverted := Inverted subclass.
        TestInverted addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := val + 1 ].

        val                 := 0.
        invertedInstance1   := TestInverted new.
        invertedInstance2   := TestInverted new.
        object              := Object new.

        invertedInstance1 receiveFrom: all.
        invertedInstance2 receiveFrom: all unreceived.
        object invertedMessage.

        val
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should set the 'val' variable to 2 as both messages should get processed as the 'unreceived' version of the message should be processed first
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(2));
    });
}

#[test]
fn send_inverted_message_to_several_targets_in_order_1() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted1 TestInverted2 invertedInstance1 invertedInstance2 object val |

        TestInverted1 := Inverted subclass.
        TestInverted1 addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := 10 ].

        TestInverted2 := Inverted subclass.
        TestInverted2 addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := 20 ].

        val                 := 0.
        invertedInstance1   := TestInverted1 new.
        invertedInstance2   := TestInverted2 new.
        object              := Object new.

        invertedInstance1 receiveFrom: object.
        invertedInstance2 receiveFrom: object.
        object invertedMessage.

        val
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should call invertedInstance2 then 1, so the final value is 10 (reverse order for the receiveFrom requests)
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(10));
    });
}

#[test]
fn send_inverted_message_to_several_targets_in_order_2() {
    // Create an inverted subclass and send a message to it from a 'normal' object
    let test_source = "
        | TestInverted1 TestInverted2 invertedInstance1 invertedInstance2 object val |

        TestInverted1 := Inverted subclass.
        TestInverted1 addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := 10 ].

        TestInverted2 := Inverted subclass.
        TestInverted2 addInvertedMessage: #invertedMessage withAction: [ :sender :self | val := 20 ].

        val                 := 0.
        invertedInstance1   := TestInverted1 new.
        invertedInstance2   := TestInverted2 new.
        object              := Object new.

        invertedInstance2 receiveFrom: object.
        invertedInstance1 receiveFrom: object.
        object invertedMessage.

        val
    ";

    executor::block_on(async { 
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        // Should call invertedInstance1 then 2, so the final value is 20
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(20));
    });
}

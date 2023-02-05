use flo_talk::*;

use futures::executor;

#[test]
fn unary_conversion() {
    let msg: TalkMessageSignatureId = "test".into();

    assert!(msg.to_signature().is_unary());
    assert!(msg.to_signature().len() == 0);
}

#[test]
fn keyword_conversion() {
    let msg: TalkMessageSignatureId = "test:".into();

    assert!(!msg.to_signature().is_unary());
    assert!(msg.to_signature().len() == 1);
}

#[test]
fn binary_symbol_conversion() {
    let msg: TalkMessageSignatureId = "+".into();

    assert!(!msg.to_signature().is_unary());
    assert!(msg.to_signature().len() == 1);
}

#[test]
fn multi_arg_conversion() {
    let msg: TalkMessageSignatureId = ("test:", "anotherTest:").into();

    assert!(!msg.to_signature().is_unary());
    assert!(msg.to_signature().len() == 2);
}

#[test]
fn unsupported_message() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("((#signature:) withArguments: #(1)) unsupported")).await;

        println!("{:?}", msg);
        assert!(msg.is_error());
    })
}

#[test]
fn unary_signature_to_message() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("#signature asMessage")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Message(Box::new(TalkMessage::unary("signature"))));
    })
}

#[test]
fn argument_signature_to_message_1() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("#signature: with: 42")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Message(Box::new(TalkMessage::with_arguments(vec![("signature:", 42)]))));
    })
}

#[test]
fn argument_signature_to_message_2() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("#signature:other: with: 1 with: 2")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Message(Box::new(TalkMessage::with_arguments(vec![("signature:", 1), ("other:", 2)]))));
    })
}

#[test]
fn argument_signature_to_message_3() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("#signature:other: withArguments: #(1 2)")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Message(Box::new(TalkMessage::with_arguments(vec![("signature:", 1), ("other:", 2)]))));
    })
}

#[test]
fn matches_selector_1() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) matchesSelector: #signature:other:")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Bool(true));
    })
}

#[test]
fn matches_selector_2() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) matchesSelector: #differentSignature:param:")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Bool(false));
    })
}

#[test]
fn argument_at() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) argumentAt: 1")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(2));
    })
}

#[test]
fn arguments() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) arguments")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Array(vec![TalkValue::Int(1), TalkValue::Int(2)]));
    })
}

#[test]
fn selector() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) selector")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Selector(("signature:", "other:").into()));
    })
}

#[test]
fn selector_starts_with_1() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#someMessage:two:three:four: withArguments: #(1 2 3 4)) selectorStartsWith: #someMessage:two:")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Bool(true));
    })
}

#[test]
fn selector_starts_with_2() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#someMessage:two:three:four: withArguments: #(1 2 3 4)) selectorStartsWith: #someMessage:notTwo:")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Bool(false));
    })
}

#[test]
fn selector_message_after() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#someMessage:two:three:four: withArguments: #(1 2 3 4)) messageAfter: #someMessage:two:")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Message(Box::new(TalkMessage::with_arguments(vec![("three:", 3), ("four:", 4)]))));
    })
}

#[test]
fn selector_message_after_match_all() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#someMessage:two:three:four: withArguments: #(1 2 3 4)) messageAfter: #someMessage:two:three:four:")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Nil);
    })
}

#[test]
fn selector_message_after_match_unary() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#unaryMessage asMessage) messageAfter: #unaryMessage")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Nil);
    })
}

#[test]
fn selector_message_combined_with() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#someMessage:two: withArguments: #(1 2)) messageCombinedWith: (#three:four: with: 3 with: 4)")).await;

        println!("{:?}", msg);

        if let TalkValue::Message(msg) = &*msg {
            println!("{:?}", msg.signature());
            assert!(msg.signature().id() == ("someMessage:", "two:", "three:", "four:").into());
        }

        assert!(*msg == TalkValue::Message(Box::new(TalkMessage::with_arguments(vec![("someMessage:", 1), ("two:", 2), ("three:", 3), ("four:", 4)]))));
    })
}

#[test]
fn if_matches_do_1() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) ifMatches: #signature:other: do: [ :one :two | one + two ]")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(3));
    })
}

#[test]
fn if_matches_do_2() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) ifMatches: #signature:notOther: do: [ :one :two | one + two ]")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Nil);
    })
}

#[test]
fn if_matches_do_otherwise_1() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) ifMatches: #signature:other: do: [ :one :two | one + two ] ifDoesNotMatch: [ 42 ]")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(3));
    })
}

#[test]
fn if_matches_do_otherwise_2() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) ifMatches: #signature:notOther: do: [ :one :two | one + two ] ifDoesNotMatch: [ 42 ]")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn if_does_not_match_1() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) ifDoesNotMatch: #signature:other: do: [ 42 ]")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Nil);
    })
}

#[test]
fn if_does_not_match_2() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) ifDoesNotMatch: #signature:notOther: do: [ 42 ]")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn multi_match_1() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("
            (#signature:other: withArguments: #(1 2)) 
                ifMatches: #test1 do: [ 100 ]
                ifMatches: #signature:other: do: [ :one :two | one + two ]
                ifMatches: #test2:withArg: do: [ :one :two | 100 + one ]
                ifDoesNotMatch: [ 42 ]
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(3));
    })
}

#[test]
fn multi_match_2() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("
            (#signature:other: withArguments: #(1 2)) 
                ifMatches: #test1 do: [ 100 ]
                ifMatches: #anotherSig:other: do: [ :one :two | one + two ]
                ifMatches: #test2:withArg: do: [ :one :two | 100 + one ]
                ifDoesNotMatch: [ 42 ]
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(42));
    })
}

#[test]
fn multi_match_3() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("
            (#signature:other: withArguments: #(1 2)) 
                ifMatches: #test1 do: [ 100 ]
                ifMatches: #signature:other: do: [ :one :two | one + two ]
                ifMatches: #test2:withArg: do: [ :one :two | 100 + one ]
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(3));
    })
}

#[test]
fn multi_match_4() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("
            (#signature:other: withArguments: #(1 2)) 
                ifMatches: #test1 do: [ 100 ]
                ifMatches: #signature:other: do: [ :one :two | one + two ]
                ifMatches: #test2:withArg: do: [ :one :two | 100 + one ]
                ifDoesNotMatch: [ 42 ].

            (#signature:other: withArguments: #(1 2)) 
                ifMatches: #test1 do: [ 100 ]
                ifMatches: #signature:other: do: [ :one :two | one + two ]
                ifMatches: #test2:withArg: do: [ :one :two | 100 + one ]
                ifDoesNotMatch: [ 42 ].

            (#signature:other: withArguments: #(1 2)) 
                ifMatches: #test1 do: [ 100 ]
                ifMatches: #signature:other: do: [ :one :two | one + two ]
                ifMatches: #test2:withArg: do: [ :one :two | 100 + one ]
                ifDoesNotMatch: [ 42 ]
        ")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Int(3));
    })
}

#[test]
fn message_is_not_nil() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        let msg     = runtime.run(TalkScript::from("(#signature:other: withArguments: #(1 2)) isNil")).await;

        println!("{:?}", msg);
        assert!(*msg == TalkValue::Bool(false));
    })
}

#[test]
fn add_parameter_to_unary_message() {
    executor::block_on(async {
        let msg = TalkMessage::unary("test");
        let msg = msg.with_extra_parameter("foo:", 0);

        println!("{:?}", msg);
        assert!(msg == TalkMessage::with_arguments(vec![("testFoo:", 0)]));
    })
}

#[test]
fn add_parameter_to_argument_message() {
    executor::block_on(async {
        let msg = TalkMessage::with_arguments(vec![("foo:", 0)]);
        let msg = msg.with_extra_parameter("bar:", 1);

        println!("{:?}", msg);
        assert!(msg == TalkMessage::with_arguments(vec![("foo:", 0), ("bar:", 1)]));
    })
}

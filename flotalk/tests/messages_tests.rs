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

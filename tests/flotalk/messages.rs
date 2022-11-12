use flo_scene::flotalk::*;

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
fn multi_arg_conversion() {
    let msg: TalkMessageSignatureId = ("test:", "anotherTest:").into();

    assert!(!msg.to_signature().is_unary());
    assert!(msg.to_signature().len() == 2);
}

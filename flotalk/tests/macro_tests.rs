use flo_talk::*;

#[derive(TalkMessageType)]
enum TestEnum {
    Int(i64),
    Float(f64),
}

#[test]
fn test_enum_int_to_message() {
    let mut context     = TalkContext::empty();
    let int_as_message  = TestEnum::Int(42).to_message(&mut context);

    assert!(int_as_message.signature_id() == "withInt:".into());
}

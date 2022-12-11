use flo_talk::*;

#[derive(TalkMessageType)]
enum TestEnum {
    Int(i64),
    Float(f64),
    ManyInts(i64, i64),
}

#[test]
fn test_enum_int_to_message() {
    let mut context     = TalkContext::empty();
    let int_as_message  = TestEnum::Int(42).to_message(&mut context);

    assert!(int_as_message.signature_id() == "withInt:".into());
}

#[test]
fn test_enum_float_to_message() {
    let mut context     = TalkContext::empty();
    let int_as_message  = TestEnum::Float(42.0).to_message(&mut context);

    assert!(int_as_message.signature_id() == "withFloat:".into());
}

#[test]
fn test_enum_many_ints_to_message() {
    let mut context     = TalkContext::empty();
    let int_as_message  = TestEnum::ManyInts(1, 2).to_message(&mut context);

    assert!(int_as_message.signature_id() == ("withManyInts:", ":").into());
}

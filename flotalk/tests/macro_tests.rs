use flo_talk::*;

#[derive(TalkMessageType, PartialEq)]
enum TestEnum {
    Unary,
    Int(i64),
    Float(f64),
    ManyInts(i64, i64),
}

#[test]
fn test_enum_unary_to_message() {
    let mut context         = TalkContext::empty();
    let unary_as_message    = TestEnum::Unary.to_message(&context);

    assert!(unary_as_message.signature_id() == "withUnary".into());
}

#[test]
fn test_enum_int_to_message() {
    let mut context     = TalkContext::empty();
    let int_as_message  = TestEnum::Int(42).to_message(&context);

    assert!(int_as_message.signature_id() == "withInt:".into());
}

#[test]
fn test_enum_float_to_message() {
    let mut context         = TalkContext::empty();
    let float_as_message    = TestEnum::Float(42.0).to_message(&context);

    assert!(float_as_message.signature_id() == "withFloat:".into());
}

#[test]
fn test_enum_many_ints_to_message() {
    let mut context     = TalkContext::empty();
    let int_as_message  = TestEnum::ManyInts(1, 2).to_message(&context);

    assert!(int_as_message.signature_id() == ("withManyInts:", ":").into());
}

#[test]
fn test_enum_unary_from_message() {
    let mut context         = TalkContext::empty();
    let unary_as_message    = TestEnum::Unary.to_message(&context);
    let unary_as_unary      = TestEnum::from_message(unary_as_message, &context).unwrap();

    assert!(unary_as_unary == TestEnum::Unary);
}

#[test]
fn test_enum_int_from_message() {
    let mut context     = TalkContext::empty();
    let int_as_message  = TestEnum::Int(42).to_message(&context);
    let int_as_int      = TestEnum::from_message(int_as_message, &context).unwrap();

    assert!(int_as_int == TestEnum::Int(42));
}

#[test]
fn test_enum_many_ints_from_message() {
    let mut context     = TalkContext::empty();
    let int_as_message  = TestEnum::ManyInts(1, 2).to_message(&context);
    let int_as_int      = TestEnum::from_message(int_as_message, &context).unwrap();

    assert!(int_as_int == TestEnum::ManyInts(1, 2));
}

use flo_talk::*;
use smallvec::*;

#[derive(TalkMessageType, PartialEq)]
enum TestEnum {
    Unary,
    Int(i64),
    Float(f64),
    ManyInts(i64, i64),
    Structured {
        one: i64,
        two: i64,
    },
    EmptyUnstructured(),
    EmptyStructured { },
}

#[derive(TalkMessageType, PartialEq)]
enum TestEnumRecursive {
    Int(i64),
    AnotherEnum(TestEnum, i64),   
}

#[test]
fn test_enum_unary_to_message() {
    let context             = TalkContext::empty();
    let unary_as_message    = TestEnum::Unary.to_message(&context);

    assert!(unary_as_message.signature_id() == "withUnary".into());
}

#[test]
fn test_enum_empty_structured_to_message() {
    let context             = TalkContext::empty();
    let unary_as_message    = TestEnum::EmptyStructured { }.to_message(&context);

    assert!(unary_as_message.signature_id() == "withEmptyStructured".into());
}

#[test]
fn test_enum_empty_unstructured_to_message() {
    let context             = TalkContext::empty();
    let unary_as_message    = TestEnum::EmptyUnstructured().to_message(&context);

    assert!(unary_as_message.signature_id() == "withEmptyUnstructured".into());
}

#[test]
fn test_enum_int_to_message() {
    let context         = TalkContext::empty();
    let int_as_message  = TestEnum::Int(42).to_message(&context);

    assert!(int_as_message.signature_id() == "withInt:".into());
}

#[test]
fn test_enum_float_to_message() {
    let context             = TalkContext::empty();
    let float_as_message    = TestEnum::Float(42.0).to_message(&context);

    assert!(float_as_message.signature_id() == "withFloat:".into());
}

#[test]
fn test_enum_many_ints_to_message() {
    let context         = TalkContext::empty();
    let int_as_message  = TestEnum::ManyInts(1, 2).to_message(&context);

    assert!(int_as_message.signature_id() == ("withManyInts:", ":").into());
}

#[test]
fn test_enum_structured_to_message() {
    let context                 = TalkContext::empty();
    let structured_as_message   = TestEnum::Structured { one: 1, two: 2 }.to_message(&context);

    assert!(structured_as_message.signature_id() == ("withStructuredOne:", "two:").into());
}

#[test]
fn test_enum_unary_from_message() {
    let context             = TalkContext::empty();
    let unary_as_message    = TestEnum::Unary.to_message(&context);
    let unary_as_unary      = TestEnum::from_message(unary_as_message, &context).unwrap();

    assert!(unary_as_unary == TestEnum::Unary);
}

#[test]
fn test_enum_int_from_message_1() {
    let context         = TalkContext::empty();
    let int_as_message  = TestEnum::Int(42).to_message(&context);
    let int_as_int      = TestEnum::from_message(int_as_message, &context).unwrap();

    assert!(int_as_int == TestEnum::Int(42));
}

#[test]
fn test_enum_int_from_message_2() {
    let context         = TalkContext::empty();
    let int_as_message  = TalkMessage::WithArguments("withInt:".into(), smallvec![42.into()]);
    let int_as_message  = TalkOwned::new(int_as_message, &context);
    let int_as_int      = TestEnum::from_message(int_as_message, &context).unwrap();

    assert!(int_as_int == TestEnum::Int(42));
}

#[test]
fn test_enum_many_ints_from_message() {
    let context         = TalkContext::empty();
    let int_as_message  = TestEnum::ManyInts(1, 2).to_message(&context);
    let int_as_int      = TestEnum::from_message(int_as_message, &context).unwrap();

    assert!(int_as_int == TestEnum::ManyInts(1, 2));
}

#[test]
fn test_enum_structured_from_message() {
    let context                     = TalkContext::empty();
    let structured_as_message       = TestEnum::Structured { one: 1, two: 2 }.to_message(&context);
    let structured_as_structured    = TestEnum::from_message(structured_as_message, &context).unwrap();

    assert!(structured_as_structured == TestEnum::Structured { one: 1, two: 2 });
}

#[test]
fn test_enum_empty_structured_from_message() {
    let context             = TalkContext::empty();
    let unary_as_message    = TestEnum::EmptyStructured { }.to_message(&context);
    let back_to_enum        = TestEnum::from_message(unary_as_message, &context).unwrap();

    assert!(back_to_enum == TestEnum::EmptyStructured { });
}

#[test]
fn test_enum_empty_unstructured_from_message() {
    let context             = TalkContext::empty();
    let unary_as_message    = TestEnum::EmptyUnstructured().to_message(&context);
    let back_to_enum        = TestEnum::from_message(unary_as_message, &context).unwrap();

    assert!(back_to_enum == TestEnum::EmptyUnstructured());
}

#[test]
fn test_enum_recursive_from_message_1() {
    let context         = TalkContext::empty();
    let message         = TestEnumRecursive::AnotherEnum(TestEnum::ManyInts(1, 2), 42).to_message(&context);
    let back_to_enum    = TestEnumRecursive::from_message(message, &context).unwrap();

    assert!(back_to_enum == TestEnumRecursive::AnotherEnum(TestEnum::ManyInts(1, 2), 42));
}

#[test]
fn test_enum_recursive_from_message_2() {
    let context         = TalkContext::empty();
    let message         = TestEnumRecursive::AnotherEnum(TestEnum::Int(1), 42).to_message(&context);
    let back_to_enum    = TestEnumRecursive::from_message(message, &context).unwrap();

    assert!(back_to_enum == TestEnumRecursive::AnotherEnum(TestEnum::Int(1), 42));
}

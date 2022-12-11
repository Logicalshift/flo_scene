use flo_talk::*;

use std::sync::*;

#[test]
fn integer_number() {
    assert!(TalkValue::try_from(TalkLiteral::Number(Arc::new("42".into()))) == Ok(TalkValue::Int(42)));
}

#[test]
fn float_number() {
    assert!(TalkValue::try_from(TalkLiteral::Number(Arc::new("0.42".into()))) == Ok(TalkValue::Float(0.42)));
}

#[test]
fn radix_number() {
    assert!(TalkValue::try_from(TalkLiteral::Number(Arc::new("16rF00D".into()))) == Ok(TalkValue::Int(0xf00d)));
}

#[test]
fn string() {
    assert!(TalkValue::try_from(TalkLiteral::String(Arc::new("test".into()))) == Ok(TalkValue::String(Arc::new("test".into()))));
}

#[test]
fn symbol() {
    assert!(TalkValue::try_from(TalkLiteral::Symbol(Arc::new("some_symbol".into()))) == Ok(TalkValue::Symbol("some_symbol".into())));
}

#[test]
fn selector() {
    assert!(TalkValue::try_from(TalkLiteral::Selector(vec![Arc::new("some_symbol:".into())])) == Ok(TalkValue::Selector("some_symbol:".into())));
}

#[test]
fn array() {
    assert!(TalkValue::try_from(TalkLiteral::Array(vec![
            TalkLiteral::Number(Arc::new("42".into())),
            TalkLiteral::Number(Arc::new("43".into())),
            TalkLiteral::Number(Arc::new("44".into())),
        ])) == Ok(TalkValue::Array(vec![
            TalkValue::Int(42),
            TalkValue::Int(43),
            TalkValue::Int(44),
        ])));
}


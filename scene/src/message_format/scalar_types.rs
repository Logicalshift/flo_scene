use super::message_format::*;
use super::message_format_trait::*;

impl HasMessageFormat for u8 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Unsigned(8).into())
    }
}

impl HasMessageFormat for u16 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Unsigned(16).into())
    }
}

impl HasMessageFormat for u32 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Unsigned(32).into())
    }
}

impl HasMessageFormat for u64 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Unsigned(64).into())
    }
}

impl HasMessageFormat for u128 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Unsigned(128).into())
    }
}

impl HasMessageFormat for usize {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Unsigned(64).into())
    }
}

impl HasMessageFormat for i8 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Signed(8).into())
    }
}

impl HasMessageFormat for i16 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Signed(16).into())
    }
}

impl HasMessageFormat for i32 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Signed(32).into())
    }
}

impl HasMessageFormat for i64 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Signed(64).into())
    }
}

impl HasMessageFormat for i128 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Signed(128).into())
    }
}

impl HasMessageFormat for isize {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Signed(64).into())
    }
}

impl HasMessageFormat for f32 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Float(32).into())
    }
}

impl HasMessageFormat for f64 {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Float(64).into())
    }
}

impl HasMessageFormat for bool {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Boolean.into())
    }
}

impl HasMessageFormat for char {
    fn message_format() -> Option<MessageFormat> {
        Some(ScalarType::Character.into())
    }
}

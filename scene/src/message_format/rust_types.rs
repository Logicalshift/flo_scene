use super::message_format::*;
use super::message_format_trait::*;

impl HasMessageFormat for String {
    fn message_format() -> Option<MessageFormat> {
        Some(FormatDescriptor::String.into())
    }
}

impl<T> HasMessageFormat for Vec<T>
where
    T: HasMessageFormat
{
    fn message_format() -> Option<MessageFormat> {
        Some(FormatDescriptor::Array(Box::new(T::message_format()?)).into())
    }
}

impl<T> HasMessageFormat for [T]
where
    T: HasMessageFormat
{
    fn message_format() -> Option<MessageFormat> {
        Some(FormatDescriptor::Array(Box::new(T::message_format()?)).into())
    }
}

use super::message_format::*;
use super::message_format_trait::*;

use std::time::{Duration};

/* -- can't implement both HasMessageFormat and SceneMessage on the same type
impl HasMessageFormat for String {
    fn message_format() -> Option<MessageFormat> {
        Some(FormatDescriptor::String.into())
    }
}
*/

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

impl HasMessageFormat for Duration {
    fn message_format() -> Option<MessageFormat> {
        // Serde serializes these as 'as_secs()' and 'subsec_nanos()' which gives us a u64 and a u32

        Some(FormatDescriptor::Tuple(vec![
            u64::message_format()?.into(),
            u32::message_format()?.into(),
        ]).into())
    }
}

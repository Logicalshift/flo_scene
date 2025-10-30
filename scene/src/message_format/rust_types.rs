use flo_scene_guest::*;

use super::message_format::*;
use super::message_format_trait::*;

use uuid::{Uuid};

use std::time::{Duration};

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

impl HasMessageFormat for Duration {
    fn message_format() -> Option<MessageFormat> {
        // Serde serializes these as 'as_secs()' and 'subsec_nanos()' which gives us a u64 and a u32

        Some(FormatDescriptor::Tuple(vec![
            u64::message_format()?.into(),
            u32::message_format()?.into(),
        ]).into())
    }
}

impl HasMessageFormat for Uuid {
    fn message_format() -> Option<MessageFormat> {
        Some(FormatDescriptor::Array(Box::new(u8::message_format().unwrap())).into())
    }
}

impl HasMessageFormat for SubProgramId {
    fn message_format() -> Option<MessageFormat> {
        Some(
            FormatDescriptor::Enum(vec![
                Variant {
                    name:           "Named".into(),
                    argument_type:  FormatDescriptor::String.into(),
                },

                Variant {
                    name:           "Guid".into(),
                    argument_type:  Uuid::message_format().unwrap().into(),
                },

                Variant {
                    name:           "NamedTask".into(),
                    argument_type:  FormatDescriptor::Tuple(vec![FormatDescriptor::String.into(), usize::message_format().unwrap()]).into(),
                },

                Variant {
                    name:           "GuidTask".into(),
                    argument_type:  FormatDescriptor::Tuple(vec![Uuid::message_format().unwrap().into(), usize::message_format().unwrap()]).into(),
                },
            ]).into())
    }
}
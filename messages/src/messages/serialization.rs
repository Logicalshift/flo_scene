use super::stream_id::*;
use super::traits::*;
use crate::no_std::*;

use alloc::borrow::*;
use core::any::*;

/// The known type names of serialized types
static SERIALIZABLE_MESSAGE_TYPE_NAMES: StaticLazy<Shared<OrderedVec<TypeId, Cow<'static, str>>>> = StaticLazy::new(|| share(OrderedVec::new()));

/// The stream ID for a known serializable type
static STREAM_ID_FOR_SERIALIZABLE_TYPE: StaticLazy<Shared<OrderedVec<Cow<'static, str>, StreamId>>> = StaticLazy::new(|| share(OrderedVec::new()));

/// The stream ID for a rust type name
static STREAM_ID_FOR_RUST_TYPE: StaticLazy<Shared<OrderedVec<Cow<'static, str>, StreamId>>> = StaticLazy::new(|| share(OrderedVec::new()));

impl StreamId {
    #[inline]
    pub (super) fn install_serialization_names<TMessageType>(&self)
    where
        TMessageType: SceneMessage,
    {
        let type_id = TypeId::of::<TMessageType>();

        if !with_shared(&*SERIALIZABLE_MESSAGE_TYPE_NAMES, |type_names| type_names.contains(&type_id)) {
            let type_name: Cow<'static, str> = TMessageType::message_type_name().into();

            with_shared(&*SERIALIZABLE_MESSAGE_TYPE_NAMES,  |stream_ids| stream_ids.entry(type_id).or_insert_with(|| type_name.clone()));
            with_shared(&*STREAM_ID_FOR_SERIALIZABLE_TYPE,  |stream_ids| stream_ids.entry(type_name.clone()).or_insert_with(|| self.clone()));
            with_shared(&*STREAM_ID_FOR_RUST_TYPE,          |stream_ids| stream_ids.entry(type_name.clone()).or_insert_with(|| self.clone()));
        }
    }

    ///
    /// If this stream can be serialized, then this is the serialization type name that can be used to specify it
    ///
    pub fn serialization_type_name(&self) -> Option<Cow<'static, str>> {
        with_shared(&*SERIALIZABLE_MESSAGE_TYPE_NAMES, |type_names| type_names.get(&self.message_type()).cloned())
    }

    ///
    /// Changes a serialization name into a stream ID
    ///
    pub fn with_serialization_type(type_name: impl AsRef<str>) -> Option<Self> {
        with_shared(&*STREAM_ID_FOR_SERIALIZABLE_TYPE, |stream_ids| stream_ids.get(type_name.as_ref()).cloned())
    }

    ///
    /// Changes a serialization name into a stream ID
    ///
    pub fn with_rust_type(type_name: impl AsRef<str>) -> Option<Self> {
        with_shared(&*STREAM_ID_FOR_RUST_TYPE, |stream_ids| stream_ids.get(type_name.as_ref()).cloned())
    }
}

use super::stream_id::*;
use super::traits::*;

use std::any::*;
use std::borrow::*;
use std::collections::*;
use std::sync::*;

/// The known type names of serialized types
static SERIALIZABLE_MESSAGE_TYPE_NAMES: LazyLock<RwLock<HashMap<TypeId, Cow<'static, str>>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// The stream ID for a known serializable type
static STREAM_ID_FOR_SERIALIZABLE_TYPE: LazyLock<RwLock<HashMap<Cow<'static, str>, StreamId>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// The stream ID for a rust type name
static STREAM_ID_FOR_RUST_TYPE: LazyLock<RwLock<HashMap<Cow<'static, str>, StreamId>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

impl StreamId {
    #[inline]
    pub (super) fn install_serialization_names<TMessageType>(&self)
    where
        TMessageType: SceneMessage,
    {
        let type_id = TypeId::of::<TMessageType>();

        if !SERIALIZABLE_MESSAGE_TYPE_NAMES.read().unwrap().contains_key(&type_id) {
            let type_name: Cow<'static, str> = TMessageType::message_type_name().into();

            SERIALIZABLE_MESSAGE_TYPE_NAMES.write().unwrap().entry(type_id).or_insert_with(|| type_name.clone());
            STREAM_ID_FOR_SERIALIZABLE_TYPE.write().unwrap().entry(type_name.clone()).or_insert_with(|| self.clone());
            STREAM_ID_FOR_RUST_TYPE.write().unwrap().entry(type_name.clone()).or_insert_with(|| self.clone());
        }
    }

    ///
    /// If this stream can be serialized, then this is the serialization type name that can be used to specify it
    ///
    pub fn serialization_type_name(&self) -> Option<Cow<'static, str>> {
        (*SERIALIZABLE_MESSAGE_TYPE_NAMES).read().unwrap().get(&self.message_type()).cloned()
    }

    ///
    /// Changes a serialization name into a stream ID
    ///
    pub fn with_serialization_type(type_name: impl Into<Cow<'static, str>>) -> Option<Self> {
        (*STREAM_ID_FOR_SERIALIZABLE_TYPE).read().unwrap().get(&type_name.into()).cloned()
    }

    ///
    /// Changes a serialization name into a stream ID
    ///
    pub fn with_rust_type(type_name: impl Into<Cow<'static, str>>) -> Option<Self> {
        (*STREAM_ID_FOR_RUST_TYPE).read().unwrap().get(&type_name.into()).cloned()
    }
}

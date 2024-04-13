use crate::filter::*;
use crate::input_stream::*;
use crate::scene_message::*;

use futures::prelude::*;
use futures::stream;
use futures::stream::{BoxStream};

use serde::*;

use std::any::*;
use std::sync::*;

///
/// A message created by serializing another message
///
/// The type ID here can be used if it's necessary to deserialize the message again or determine the original type that was serialized.
///
pub struct SerializedMessage<TSerializedType>(pub TSerializedType, pub TypeId);

impl<TSerializedType> SceneMessage for SerializedMessage<TSerializedType> 
where
    TSerializedType: Send + Unpin,
{
}

///
/// Creates a filter that will serialize a message of the specified type
///
/// The filter generated here will create `SerializedMessage` messages, mapped to a final output type via the map_stream message. This example
/// leaves the message as a 'SerializedMessage':
///
/// ```
/// # use flo_scene::*;
/// # use flo_scene::programs::*;
/// #
/// # use serde::*;
/// # use serde_json;
/// #
/// # #[derive(Serialize)]
/// # enum TestMessage { Test }
/// # impl SceneMessage for TestMessage { }
/// let serialize_filter = serializer_filter::<TestMessage, _, _>(|| serde_json::value::Serializer, |stream| stream);
/// ```
///
pub fn serializer_filter<TMessageType, TSerializer, TTargetStream>(serializer: impl 'static + Send + Sync + Fn() -> TSerializer, map_stream: impl 'static + Send + Sync + Fn(BoxStream<'static, SerializedMessage<TSerializer::Ok>>) -> TTargetStream) -> FilterHandle
where
    TMessageType:           'static + SceneMessage + Serialize,
    TSerializer:            'static + Send + Serializer,
    TSerializer::Ok:        'static + Send + Unpin,
    TTargetStream:          'static + Send + Stream,
    TTargetStream::Item:    'static + SceneMessage,
{
    // Create a serializer
    let serializer  = Arc::new(serializer);
    let type_id     = TypeId::of::<TMessageType>();

    // 
    FilterHandle::for_filter(move |message_stream: InputStream<TMessageType>| {
        let serializer = serializer.clone();

        let serialized_stream = message_stream
            .map(move |message| {
                let serializer  = (serializer)();
                let serialized  = message.serialize(serializer).ok()
                    .map(|serialized| SerializedMessage(serialized, type_id));

                stream::iter(serialized)
            })
            .flatten()
            .boxed();

        map_stream(serialized_stream)
    })
}

use crate::error::*;
use crate::filter::*;
use crate::input_stream::*;
use crate::scene::*;
use crate::scene_message::*;
use crate::stream_source::*;
use crate::stream_id::*;

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
/// If a message generates an error when serialized, this will ignore it.
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
/// #
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
    let serializer  = Arc::new(serializer);
    let type_id     = TypeId::of::<TMessageType>();

    // The filter creates a serializer per message, then passes the stream through the `map_stream` function to generate the final message type
    // map_stream is here because otherwise it's quite hard to accept serialized messages along with other types as we can't combine filters
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

///
/// Creates a filter that can be used to deserialize incoming messages of a particular type
///
/// The mapping stream can be used to further change the message type if neeeded.
///
/// If a message has the wrong type ID attached to it, or generates an error when deserializing, this will ignore it.
///
/// ```
/// # use flo_scene::*;
/// # use flo_scene::programs::*;
/// #
/// # use serde::*;
/// # use serde_json;
/// #
/// # #[derive(Serialize, Deserialize)]
/// # enum TestMessage { Test }
/// # impl SceneMessage for TestMessage { }
/// #
/// let deserialize_filter = deserializer_filter::<TestMessage, serde_json::Value, _>(|stream| stream);
/// ```
///
pub fn deserializer_filter<TMessageType, TSerializedValue, TTargetStream>(map_stream: impl 'static + Send + Sync + Fn(BoxStream<'static, TMessageType>) -> TTargetStream) -> FilterHandle
where
    TMessageType:           'static + SceneMessage + for<'a> Deserialize<'a>,
    TSerializedValue:       'static + Send + Unpin + for<'a> Deserializer<'a>,
    TTargetStream:          'static + Send + Stream,
    TTargetStream::Item:    'static + SceneMessage,
{
    let type_id     = TypeId::of::<TMessageType>();

    FilterHandle::for_filter(move |message_stream: InputStream<SerializedMessage<TSerializedValue>>| {
        let deserialized_stream = message_stream
            .map(move |SerializedMessage(message_value, message_type)| {
                if message_type != type_id {
                    stream::iter(None)
                } else {
                    stream::iter(TMessageType::deserialize(message_value).ok())
                }
            })
            .flatten()
            .boxed();

        map_stream(deserialized_stream)
    })
}

///
/// Install serializers and deserializers so that messages of a particular type can be filtered to and from `SerializedMessage<TSerializer::Ok>`
///
pub fn install_serializers<TMessageType, TSerializer>(scene: &Scene, create_serializer: impl 'static + Send + Sync + Fn() -> TSerializer) -> Result<(), ConnectionError>
where
    TMessageType:       'static + SceneMessage,
    TMessageType:       for<'a> Deserialize<'a>,
    TMessageType:       Serialize,
    TSerializer:        'static + Send + Serializer,
    TSerializer::Ok:    'static + Send + Unpin,
    TSerializer::Ok:    for<'a> Deserializer<'a>,
{
    // Create/fetch the filters for the message type
    let type_id             = TypeId::of::<TMessageType>();
    let serialize_filter    = serializer_filter::<TMessageType, _, _>(move || create_serializer(), move |stream| stream.map(move |serialized| SerializedMessage(serialized, type_id)));
    let deserialize_filter  = deserializer_filter::<TMessageType, TSerializer::Ok, _>(|stream| stream);

    // Add source filters to serialize and deserialize to the scene
    scene.connect_programs(StreamSource::Filtered(serialize_filter), (), StreamId::with_message_type::<TMessageType>())?;
    scene.connect_programs(StreamSource::Filtered(deserialize_filter), (), StreamId::with_message_type::<SerializedMessage<TSerializer::Ok>>())?;

    Ok(())
}

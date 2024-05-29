use crate::error::*;
use crate::filter::*;
use crate::input_stream::*;
use crate::scene::*;
use crate::output_sink::*;
use crate::scene_context::*;
use crate::scene_message::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::stream_id::*;

use futures::prelude::*;
use futures::stream;
use futures::stream::{BoxStream};

use once_cell::sync::{Lazy};
use serde::*;

use std::any::*;
use std::collections::{HashMap};
use std::sync::*;

static SEND_SERIALIZED: Lazy<RwLock<HashMap<(TypeId, String), Arc<dyn Send + Sync + Fn(&SceneContext, StreamTarget) -> Result<Box<dyn Send + Any>, ConnectionError>>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

///
/// A message created by serializing another message
///
/// The type ID here can be used if it's necessary to deserialize the message again or determine the original type that was serialized.
///
#[derive(Debug, PartialEq)]
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
/// The type name is associated with the filters created by this function and can be used to create a sink that sends the raw serialized messages. This name
/// must be unique: use something like `crate_name::type_name` for this value to ensure that there are no conflicts.
///
pub fn install_serializers<TMessageType, TSerializer>(scene: &Scene, type_name: &str, create_serializer: impl 'static + Send + Sync + Fn() -> TSerializer) -> Result<(), ConnectionError>
where
    TMessageType:       'static + SceneMessage,
    TMessageType:       for<'a> Deserialize<'a>,
    TMessageType:       Serialize,
    TSerializer:        'static + Send + Serializer,
    TSerializer::Ok:    'static + Send + Unpin,
    TSerializer::Ok:    for<'a> Deserializer<'a>,
{
    use std::mem;

    // Stores the currently known filters
    static FILTERS_FOR_TYPE: Lazy<RwLock<HashMap<(TypeId, TypeId), (FilterHandle, FilterHandle)>>> = 
        Lazy::new(|| RwLock::new(HashMap::new()));

    // Fetch the existing filters if there are any for this type
    let message_type        = TypeId::of::<TMessageType>();
    let serializer_type     = TypeId::of::<TSerializer>();
    let filters_for_type    = FILTERS_FOR_TYPE.read().unwrap();

    let (serialize_filter, deserialize_filter) = if let Some(filters) = filters_for_type.get(&(message_type, serializer_type)) {
        // Use the known filters
        *filters
    } else {
        // Try again with the write lock (to avoid a race condition)
        mem::drop(filters_for_type);
        let mut filters_for_type = FILTERS_FOR_TYPE.write().unwrap();

        if let Some(filters) = filters_for_type.get(&(message_type, serializer_type)) {
            // Rare race condition occurred and the filters were being created on another thread
            *filters
        } else {
            // Create some new filters for this message type
            let serialize_filter    = serializer_filter::<TMessageType, _, _>(move || create_serializer(), move |stream| stream);
            let deserialize_filter  = deserializer_filter::<TMessageType, TSerializer::Ok, _>(|stream| stream);

            // Cache them
            filters_for_type.insert((message_type, serializer_type), (serialize_filter, deserialize_filter));

            // Use them as the filters to connect
            (serialize_filter, deserialize_filter)
        }
    };

    // Create a function to generate a sink to deserialize messages of this type
    {
        let mut send_serialized = SEND_SERIALIZED.write().unwrap();
        let filter_sink         = Arc::new(|scene_context: &SceneContext, target: StreamTarget| -> Result<Box<dyn Send + Any>, ConnectionError> {
            // Create a sink to send the message type to
            let sink = scene_context.send::<TMessageType>(target)?;

            // Map it to the deserializer
            // TODO: report deserialization errors
            let sink = sink.with_flat_map(|msg: TSerializer::Ok| {
                stream::iter(TMessageType::deserialize(msg).ok().map(|msg| Ok(msg)))
            }).sink_map_err(|err| {
                // TODO: the other error types require getting back the serialized values
                SceneSendError::TargetProgramEndedBeforeReady
            });

            // Convert to an 'any' sink
            let sink: Box<dyn Unpin + Send + Sink<TSerializer::Ok, Error=SceneSendError<TSerializer::Ok>>> = Box::new(sink);
            let sink: Box<dyn Send + Any> = Box::new(sink);

            Ok(sink)
        });

        send_serialized.insert((TypeId::of::<TSerializer::Ok>(), type_name.to_string()), filter_sink);
    }

    // Add source filters to serialize and deserialize to the scene
    scene.connect_programs(StreamSource::Filtered(serialize_filter), (), StreamId::with_message_type::<TMessageType>())?;
    scene.connect_programs(StreamSource::Filtered(deserialize_filter), (), StreamId::with_message_type::<SerializedMessage<TSerializer::Ok>>())?;

    Ok(())
}

impl SceneContext {
    ///
    /// Creates an output sink that receives messages serialized using a serde serializer, and sends them using the native type.
    ///
    /// The serializer needs to be installed using `install_serializers` with a matching `type_name`.
    ///
    pub fn send_serialized<TSerializedType>(&self, type_name: &str, target: impl Into<StreamTarget>) -> Result<impl 'static + Unpin + Send + Sink<TSerializedType, Error=SceneSendError<TSerializedType>>, ConnectionError>
    where
        TSerializedType:    'static + Send + Unpin,
    {
        // Try to fetch the function that creates the sink for this type
        let send_serialized = SEND_SERIALIZED.read().unwrap();

        if let Some(create_sink) = send_serialized.get(&(TypeId::of::<TSerializedType>(), type_name.to_string())) {
            // Create a sink for the type name
            let any_sink    = (create_sink)(self, target.into())?;
            let boxed_sink  = any_sink.downcast::<Box<dyn Unpin + Send + Sink<TSerializedType, Error=SceneSendError<TSerializedType>>>>().unwrap();

            Ok(*boxed_sink)
        } else {
            // This type is not available
            Err(ConnectionError::TargetNotAvailable)
        }
    }
}

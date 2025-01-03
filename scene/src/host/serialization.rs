use crate::host::error::*;
use crate::host::filter::*;
use crate::host::scene::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::stream_target::*;
use crate::host::stream_id::*;
use crate::host::subprogram_id::*;
use crate::host::programs::*;

use futures::prelude::*;
use futures::stream;

use once_cell::sync::{Lazy};
use serde::*;
use serde::ser::{Error as SeError};
use serde::de::{Error as DeError};

use std::any::*;
use std::collections::{HashMap};
use std::marker::{PhantomData};
use std::ops::{Deref};
use std::sync::*;

/// The known type names of serialized types
static SERIALIZABLE_MESSAGE_TYPE_NAMES: Lazy<RwLock<HashMap<TypeId, String>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// The type ID assigned to a particular name (once a name is assigned to a type, it cannot be reassigned)
static TYPE_ID_FOR_NAME: Lazy<RwLock<HashMap<String, TypeId>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// The stream ID for a known serializable type
static STREAM_ID_FOR_SERIALIZABLE_TYPE: Lazy<RwLock<HashMap<String, StreamId>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// The stream ID for a rust type name
static STREAM_ID_FOR_RUST_TYPE: Lazy<RwLock<HashMap<String, StreamId>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Calls the 'send()' call and then deserializes the result
static SEND_DESERIALIZED: Lazy<RwLock<HashMap<(TypeId, TypeId), Arc<dyn Send + Sync + Fn(StreamTarget, &SceneContext) -> Result<Box<dyn Send + Any>, ConnectionError>>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Stores the functions for transforming a value to and from its serialized representation
static TYPED_SERIALIZERS: Lazy<RwLock<HashMap<(TypeId, TypeId), Arc<dyn Send + Sync + Any>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Stores the filters we've already created so we don't create extr
static FILTERS_FOR_TYPE: Lazy<Mutex<HashMap<(TypeId, TypeId), Vec<FilterHandle>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

///
/// Bytes in the guest message serialized format (this is Postcard in v0.2)
///
#[cfg(any(feature="postcard", target_family="wasm"))]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GuestMessage(pub Vec<u8>);

///
/// A message created by serializing another message
///
/// The type ID here can be used if it's necessary to deserialize the message again or determine the original type that was serialized.
///
#[derive(Debug, PartialEq)]
pub struct SerializedMessage<TSerializedType>(pub TSerializedType, pub TypeId);

impl<TSerializedType> SceneMessage for SerializedMessage<TSerializedType> 
where
    TSerializedType: 'static + Send + Unpin,
{
    fn serializable() -> bool { false }

    #[inline]
    fn message_type_name() -> String { format!("flo_scene::SerializedMessage<{}>", std::any::type_name::<TSerializedType>()) }
}

impl<TSerializedType> Serialize for SerializedMessage<TSerializedType> {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        Err(S::Error::custom("SerializedMessage cannot be serialized"))
    }
}

impl<'a, TSerializedType> Deserialize<'a> for SerializedMessage<TSerializedType> {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a> 
    {
        Err(D::Error::custom("SerializedMessage cannot be serialized"))
    }
}

///
/// Creates the data structures needed to serialize a particular type
///
/// The type name supplied here must be unique inside the process. Using `crate_name::type_name` for this value
/// is a reasonable way to guarantee uniqueness. This will return an error if a non-unique type name is used.
///
/// As well as the main type, this will generate `query::{type_name}` and `subscribe::{type_name}` to allow
/// querying or subscribing to messages of this type.
///
/// The serializer must have previously been installed with `install_serializer` so that `flo_scene` knows how to
/// create an instance of it. The type name must be unique and is associated with the serialized type: it's used
/// when deciding how to deserialize a value.
///
/// It's necessary to install a version of the serializable type for each serializer that's in use. The type name must
/// identify a single message type and cannot be used for a different `TMessageType` 
///
pub fn install_serializable_type<TMessageType, TSerializedType>(
    serialize:   impl 'static + Send + Sync + Fn(TMessageType)     -> Result<TSerializedType, SceneSendError<TMessageType>>, 
    deserialize: impl 'static + Send + Sync + Fn(&TSerializedType) -> Result<TMessageType, SceneSendError<()>>) -> Result<(), &'static str>
where
    TSerializedType:    'static + Send,
    TMessageType:       'static + SceneMessage,
{
    let type_name = TMessageType::message_type_name();

    {
        let mut type_for_name = (*TYPE_ID_FOR_NAME).write().unwrap();

        if let Some(existing_type_id) = type_for_name.get(&type_name) {
            if existing_type_id != &TypeId::of::<TMessageType>() {
                return Err("Serialization type name has been used by another type");
            }
        } else {
            type_for_name.insert(type_name.clone(), TypeId::of::<TMessageType>());
        }
    }

    {
        let mut type_names = (*SERIALIZABLE_MESSAGE_TYPE_NAMES).write().unwrap();

        if let Some(existing_type_name) = type_names.get(&TypeId::of::<TMessageType>()) {
            if existing_type_name != &type_name {
                return Err("Serialization type has already been assigned a different name");
            }
        } else {
            type_names.insert(TypeId::of::<TMessageType>(), type_name.clone());
        }
    }

    // Create closures for creating a mapping between the input and the output type
    let typed_serializer = move |input: TMessageType| -> Result<SerializedMessage<TSerializedType>, TMessageType> {
        match serialize(input) {
            Ok(val)     => Ok(SerializedMessage(val, TypeId::of::<TMessageType>())),
            Err(err)    => Err(err.to_message().unwrap())
        }
    };

    // Create another closure for deserializing
    let deserialize         = Arc::new(deserialize);
    let typed_deserialize   = Arc::clone(&deserialize);
    let typed_deserializer = move |input: SerializedMessage<TSerializedType>| -> Result<TMessageType, SerializedMessage<TSerializedType>> {
        use std::mem;

        let val = typed_deserialize(&input.0);

        match val {
            Ok(val) => Ok(val),
            Err(_)  => {
                mem::drop(val);
                Err(input)
            },
        }
    };

    // Create a closure for calling 'send()' and converting it to a sink that deserializes its input
    let send_deserialized_stream = move |target: StreamTarget, context: &SceneContext| -> Result<Box<dyn Send + Any>, ConnectionError> {
        let deserialize         = Arc::clone(&deserialize);
        let target              = context.send::<TMessageType>(target)?;
        let deserialized_target = target
            .sink_map_err(|_| SceneSendError::<TSerializedType>::ErrorAfterDeserialization)            // The error doesn't preserve the input value, so we can't return it
            .with(move |msg| future::ready(match deserialize(&msg) {
                Ok(result)  => Ok(result),
                Err(_)      => Err(SceneSendError::ErrorAfterDeserialization)
            }));

        // Box up the sink so we can use a generic type
        let boxed_target: Box<dyn Send + Unpin + Sink<TSerializedType, Error=SceneSendError::<TSerializedType>>> = Box::new(deserialized_target);

        // Box it again to make it 'Any'
        Ok(Box::new(boxed_target))
    };

    // Convert to boxed functions
    let typed_serializer: Box<dyn Send + Sync + Fn(TMessageType) -> Result<SerializedMessage<TSerializedType>, TMessageType>>                           = Box::new(typed_serializer);
    let typed_deserializer: Box<dyn Send + Sync + Fn(SerializedMessage<TSerializedType>) -> Result<TMessageType, SerializedMessage<TSerializedType>>>   = Box::new(typed_deserializer);

    // Set as an 'any' type for storage
    let typed_serializer: Arc<dyn Send + Sync + Any>    = Arc::new(typed_serializer);
    let typed_deserializer: Arc<dyn Send + Sync + Any>  = Arc::new(typed_deserializer);

    // Store the serializer and deserializer in the typed serializers list
    {
        let mut typed_serializers = (*TYPED_SERIALIZERS).write().unwrap();

        typed_serializers.insert((TypeId::of::<TMessageType>(), TypeId::of::<SerializedMessage<TSerializedType>>()), typed_serializer);
        typed_serializers.insert((TypeId::of::<SerializedMessage<TSerializedType>>(), TypeId::of::<TMessageType>()), typed_deserializer);
    }

    {
        (*STREAM_ID_FOR_SERIALIZABLE_TYPE).write().unwrap().insert(type_name.clone(), StreamId::with_message_type::<TMessageType>());
        (*STREAM_ID_FOR_RUST_TYPE).write().unwrap().insert(std::any::type_name::<TMessageType>().into(), StreamId::with_message_type::<TMessageType>());
    }

    {
        let mut send_deserialized = (*SEND_DESERIALIZED).write().unwrap();
        send_deserialized.insert((TypeId::of::<TSerializedType>(), TypeId::of::<TMessageType>()), Arc::new(send_deserialized_stream));
    }

    Ok(())
}

///
/// Returns a serialization function for changing a source type into a target type
///
/// The function returned here
///
pub fn serialization_function<TSourceType, TTargetType>() -> Result<Arc<impl 'static + Send + Fn(TSourceType) -> Result<TTargetType, TSourceType>>, &'static str>
where
    TSourceType: 'static + SceneMessage,
    TTargetType: 'static + SceneMessage,
{
    let typed_serializer = (*TYPED_SERIALIZERS).read().unwrap().get(&(TypeId::of::<TSourceType>(), TypeId::of::<TTargetType>())).cloned();
    let typed_serializer = if let Some(typed_serializer) = typed_serializer { Ok(typed_serializer) } else { Err("The requested serializers are not installed") }?;
    let typed_serializer = if let Ok(typed_serializer) = typed_serializer.downcast::<Box<dyn Send + Sync + Fn(TSourceType) -> Result<TTargetType, TSourceType>>>() { 
        Ok(typed_serializer)
    } else {
        Err("Could not properly resolve the type of the requested serializer")
    }?;

    Ok(typed_serializer)
}

///
/// If installed, returns the filters to use to convert from a source type to a target type
///
/// This will create either a serializer or a deserializer depending on the direction that the conversion goes in
///
pub fn serializer_filter<TSourceType, TTargetType>() -> Result<Vec<FilterHandle>, &'static str> 
where
    TSourceType: 'static + SceneMessage,
    TTargetType: 'static + SceneMessage,
{
    let mut filters_for_type = (*FILTERS_FOR_TYPE).lock().unwrap();

    // The message type is the key for retrieving this filter later on
    let message_type = (TypeId::of::<TSourceType>(), TypeId::of::<TTargetType>());

    if let Some(filter) = filters_for_type.get(&message_type) {
        // Use the existing filter
        Ok(filter.clone())
    } else {
        // Create a filter for converting directly between the types
        let typed_serializer = (*TYPED_SERIALIZERS).read().unwrap().get(&(TypeId::of::<TSourceType>(), TypeId::of::<TTargetType>())).cloned();
        let typed_serializer = if let Some(typed_serializer) = typed_serializer { Ok(typed_serializer) } else { Err("The requested serializers are not installed") }?;
        let typed_serializer = if let Ok(typed_serializer) = typed_serializer.downcast::<Box<dyn Send + Sync + Fn(TSourceType) -> Result<TTargetType, TSourceType>>>() { 
            Ok(typed_serializer)
        } else {
            Err("Could not properly resolve the type of the requested serializer")
        }?;

        // Create a filter that uses the stored serializer to serialize messages of this type
        let raw_type_serializer = typed_serializer.clone();
        let filter_raw_type = FilterHandle::for_filter(move |input_messages| {
            let raw_type_serializer = Arc::clone(&raw_type_serializer);

            input_messages.flat_map(move |msg| stream::iter((*raw_type_serializer)(msg).ok()))
        });

        // Create a filter that uses the stored serializer to modify query responses of this type
        let query_type_serializer = typed_serializer.clone();
        let filter_query_responses = FilterHandle::for_filter(move |input_messages| {
            let query_type_serializer = Arc::clone(&query_type_serializer);

            input_messages.map(move |response: QueryResponse<TSourceType>| {
                let query_type_serializer = Arc::clone(&query_type_serializer);
                let responses = response.flat_map(move |msg| stream::iter((*query_type_serializer)(msg).ok()));
                QueryResponse::with_stream(responses.boxed())
            })
        });

        // Store for future use
        filters_for_type.insert(message_type, vec![filter_raw_type.clone(), filter_query_responses.clone()]);

        // Result is the new filter
        Ok(vec![filter_raw_type, filter_query_responses])
    }
}

///
/// A scene being initialised with a serializer
///
pub struct SceneWithSerializer<'a, TSerializer>(&'a Scene, PhantomData<TSerializer>);

impl Scene {
    ///
    /// Starts setting up serialized types on this scene.
    ///
    pub fn with_serializer<TSerializedType>(&self) -> SceneWithSerializer<'_, TSerializedType> 
    where
        TSerializedType:    'static + Send + Unpin,
    {
        SceneWithSerializer(self, PhantomData)
    }
}

///
/// Targets for a serialized stream
///
pub enum SerializedStreamTarget {
    /// Send by deserializing to the input stream of the specified subprogram
    SubProgram(SubProgramId),

    /// Send to the default target of the specified stream
    Stream(StreamId)
}

impl From<StreamId> for SerializedStreamTarget {
    fn from(stream: StreamId) -> Self {
        SerializedStreamTarget::Stream(stream)
    }
}

impl From<SubProgramId> for SerializedStreamTarget {
    fn from(program: SubProgramId) -> Self {
        SerializedStreamTarget::SubProgram(program)
    }
}

impl SceneContext {
    ///
    /// Creates a stream to send messages using a known serialized type
    ///
    pub fn send_serialized<TMessageType>(&self, target: impl Into<SerializedStreamTarget>) -> Result<impl Sink<TMessageType, Error=SceneSendError<TMessageType>>, ConnectionError>
    where
        TMessageType: 'static + Send + Unpin + Serialize,
    {
        match target.into() {
            SerializedStreamTarget::Stream(stream_id) => {
                // Get the function for converting the 'normal' message stream into a serialized one
                let send_deserialized = (*SEND_DESERIALIZED).read().unwrap()
                    .get(&(TypeId::of::<TMessageType>(), stream_id.message_type())).cloned();
                let send_deserialized = if let Some(send_deserialized) = send_deserialized { Ok(send_deserialized) } else { Err(ConnectionError::TargetCannotDeserialize) }?;

                // Send to the default target for this message type
                let deserializer_sink = send_deserialized(StreamTarget::Any, self)?;

                // Convert to a boxed sink
                let deserializer_sink = deserializer_sink.downcast::<Box<dyn Send + Unpin + Sink<TMessageType, Error=SceneSendError::<TMessageType>>>>();

                deserializer_sink.map(|val| *val).or_else(|_| Err(ConnectionError::UnexpectedConnectionType))
            }

            SerializedStreamTarget::SubProgram(subprogram_id) => {
                // Fetch the input type of the subprogram
                let stream_id = if let Some(core) = self.scene_core().upgrade() {
                    let program     = core.lock().unwrap().get_sub_program(subprogram_id);
                    let program     = program.ok_or_else(|| ConnectionError::SubProgramNotRunning)?;
                    let stream_id   = program.lock().unwrap().input_stream_id.clone();

                    Ok(stream_id)
                } else {
                    // Nothing is running if the core is not running
                    Err(ConnectionError::SubProgramNotRunning)
                }?;

                // Send serialized data to this subprogram using this stream ID
                let target = self.send::<SerializedMessage<TMessageType>>(subprogram_id)?;
                let target = target
                    .sink_map_err(|_| SceneSendError::<TMessageType>::ErrorAfterDeserialization)            // The error doesn't preserve the input value, so we can't return it
                    .with(move |msg| 
                        future::ready(Ok(SerializedMessage(msg, stream_id.message_type()))));
                let target: Box<dyn Send + Unpin + Sink<TMessageType, Error=SceneSendError::<TMessageType>>> = Box::new(target);

                Ok(target)
            }
        }
    }
}

impl<'a, TSerializer> Deref for SceneWithSerializer<'a, TSerializer> {
    type Target = Scene;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl StreamId {
    ///
    /// If this stream can be serialized, then this is the serialization type name that can be used to specify it
    ///
    pub fn serialization_type_name(&self) -> Option<String> {
        (*SERIALIZABLE_MESSAGE_TYPE_NAMES).read().unwrap().get(&self.message_type()).cloned()
    }

    ///
    /// Changes a serialization name into a stream ID
    ///
    pub fn with_serialization_type(type_name: impl Into<String>) -> Option<Self> {
        (*STREAM_ID_FOR_SERIALIZABLE_TYPE).read().unwrap().get(&type_name.into()).cloned()
    }

    ///
    /// Changes a serialization name into a stream ID
    ///
    pub fn with_rust_type(type_name: impl Into<String>) -> Option<Self> {
        (*STREAM_ID_FOR_RUST_TYPE).read().unwrap().get(&type_name.into()).cloned()
    }
}

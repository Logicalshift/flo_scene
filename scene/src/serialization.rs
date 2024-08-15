use crate::error::*;
use crate::filter::*;
use crate::scene::*;
use crate::scene_context::*;
use crate::scene_message::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::stream_id::*;
use crate::subprogram_id::*;
use crate::programs::*;

use futures::prelude::*;
use futures::stream;

use once_cell::sync::{Lazy};
use serde::*;

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

/// Stores the functions for creating serializers of a particular type
static CREATE_ANY_SERIALIZER: Lazy<RwLock<HashMap<TypeId, Arc<dyn Send + Sync + Fn() -> Arc<dyn Send + Sync + Any>>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Calls the 'send()' call and then deserializes the result
static SEND_DESERIALIZED: Lazy<RwLock<HashMap<(TypeId, TypeId), Arc<dyn Send + Sync + Fn(StreamTarget, &SceneContext) -> Result<Box<dyn Send + Any>, ConnectionError>>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Stores the functions for transforming a value to and from its serialized representation
static TYPED_SERIALIZERS: Lazy<RwLock<HashMap<(TypeId, TypeId), Arc<dyn Send + Sync + Any>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Stores the filters we've already created so we don't create extr
static FILTERS_FOR_TYPE: Lazy<Mutex<HashMap<(TypeId, TypeId), FilterHandle>>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
/// Adds a constructor for a serializer to the types that flo_scene knows about
///
/// flo_scene can't use serializers that need setting up with state for the default way that messages are serialized,
/// but this allows it to automatically fill in all of the serializers for a single type.
///
/// This can be called multiple times for a serializer if necessary: the existing serializer will be replaced with
/// whatever is passed in.
///
pub fn install_serializer<TSerializer>(create_serializer: impl 'static + Send + Sync + Fn() -> TSerializer) 
where
    TSerializer:                    'static + Send + Serializer,
    TSerializer::Ok:                'static + Send + Unpin,
    for<'a> &'a TSerializer::Ok:    Deserializer<'a>,
{
    let mut create_any_serializer = (*CREATE_ANY_SERIALIZER).write().unwrap();

    let create_serializer_fn: Box<dyn Send + Sync + Fn() -> TSerializer>    = Box::new(create_serializer);
    let create_serializer_fn: Arc<dyn Send + Sync + Any>                    = Arc::new(create_serializer_fn);

    // Add a function that creates a boxed Any that creates this serializer type
    create_any_serializer.insert(TypeId::of::<TSerializer>(), 
        Arc::new(move || Arc::clone(&create_serializer_fn)));
}

// TODO: would be nice to not have to install the type for each type of serializable type we want to add but I'm currently not sure how to do this.
// It's probably possible if we hard-code JSON as our serialization target

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
pub fn install_serializable_type<TMessageType, TSerializer>(type_name: impl Into<String>) -> Result<(), &'static str>
where
    TMessageType:                   'static + SceneMessage,
    TMessageType:                   for<'a> Deserialize<'a>,
    TMessageType:                   Serialize,
    TSerializer:                    'static + Send + Serializer,
    TSerializer::Ok:                'static + Send + Unpin,
    for<'a> &'a TSerializer::Ok:    Deserializer<'a>,
{
    let type_name = type_name.into();

    install_single_serializable_type::<TMessageType, TSerializer>(type_name.clone())?;
    install_single_serializable_type::<Subscribe<TMessageType>, TSerializer>(format!("subscribe::{}", type_name))?;
    install_single_serializable_type::<Query<TMessageType>, TSerializer>(format!("query::{}", type_name))?;

    Ok(())
}

///
/// Like install_serializable_type but doesn't install the query/subscribe messages
///
fn install_single_serializable_type<TMessageType, TSerializer>(type_name: impl Into<String>) -> Result<(), &'static str>
where
    TMessageType:                   'static + SceneMessage,
    TMessageType:                   for<'a> Deserialize<'a>,
    TMessageType:                   Serialize,
    TSerializer:                    'static + Send + Serializer,
    TSerializer::Ok:                'static + Send + Unpin,
    for<'a> &'a TSerializer::Ok:    Deserializer<'a>,
{
    // Store the name for this type (which must match the old name)
    let type_name = type_name.into();
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

    // Fetch the serializer constructor function (this is what's set up by install_serializer)
    let new_serializer = (*CREATE_ANY_SERIALIZER).read().unwrap()
        .get(&TypeId::of::<TSerializer>())
        .cloned();
    let new_serializer = if let Some(new_serializer) = new_serializer { new_serializer } else { return Err("Serializer has not been installed by install_serializer()"); };
    let new_serializer = new_serializer().downcast::<Box<dyn Send + Sync + Fn() -> TSerializer>>();
    let new_serializer = if let Ok(new_serializer) = new_serializer { new_serializer } else { return Err("Serializer was not installed correctly"); };

    // Create closures for creating a mapping between the input and the output type
    let typed_serializer = move |input: TMessageType| -> Result<SerializedMessage<TSerializer::Ok>, TMessageType> {
        if let Ok(val) = input.serialize(new_serializer()) {
            Ok(SerializedMessage(val, TypeId::of::<TMessageType>()))
        } else {
            Err(input)
        }
    };

    // Create another closure for deserializing
    let typed_deserializer = move |input: SerializedMessage<TSerializer::Ok>| -> Result<TMessageType, SerializedMessage<TSerializer::Ok>> {
        use std::mem;

        let val = TMessageType::deserialize(&input.0);

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
        let target              = context.send::<TMessageType>(target)?;
        let deserialized_target = target
            .sink_map_err(|_| SceneSendError::<TSerializer::Ok>::ErrorAfterDeserialization)            // The error doesn't preserve the input value, so we can't return it
            .with(|msg| future::ready(match TMessageType::deserialize(&msg) {
                Ok(result)  => Ok(result),
                Err(_)      => Err(SceneSendError::ErrorAfterDeserialization)
            }));

        // Box up the sink so we can use a generic type
        let boxed_target: Box<dyn Send + Unpin + Sink<TSerializer::Ok, Error=SceneSendError::<TSerializer::Ok>>> = Box::new(deserialized_target);

        // Box it again to make it 'Any'
        Ok(Box::new(boxed_target))
    };

    // Convert to boxed functions
    let typed_serializer: Box<dyn Send + Sync + Fn(TMessageType) -> Result<SerializedMessage<TSerializer::Ok>, TMessageType>>                           = Box::new(typed_serializer);
    let typed_deserializer: Box<dyn Send + Sync + Fn(SerializedMessage<TSerializer::Ok>) -> Result<TMessageType, SerializedMessage<TSerializer::Ok>>>   = Box::new(typed_deserializer);

    // Set as an 'any' type for storage
    let typed_serializer: Arc<dyn Send + Sync + Any>    = Arc::new(typed_serializer);
    let typed_deserializer: Arc<dyn Send + Sync + Any>  = Arc::new(typed_deserializer);

    // Store the serializer and deserializer in the typed serializers list
    {
        let mut typed_serializers = (*TYPED_SERIALIZERS).write().unwrap();

        typed_serializers.insert((TypeId::of::<TMessageType>(), TypeId::of::<SerializedMessage<TSerializer::Ok>>()), typed_serializer);
        typed_serializers.insert((TypeId::of::<SerializedMessage<TSerializer::Ok>>(), TypeId::of::<TMessageType>()), typed_deserializer);
    }

    {
        (*STREAM_ID_FOR_SERIALIZABLE_TYPE).write().unwrap().insert(type_name.clone(), StreamId::with_message_type::<TMessageType>());
    }

    {
        let mut send_deserialized = (*SEND_DESERIALIZED).write().unwrap();
        send_deserialized.insert((TypeId::of::<TSerializer::Ok>(), TypeId::of::<TMessageType>()), Arc::new(send_deserialized_stream));
    }

    Ok(())
}

///
/// If installed, returns a filter to convert from a source type to a target type
///
/// This will create either a serializer or a deserializer depending on the direction that the conversion goes in
///
pub fn serializer_filter<TSourceType, TTargetType>() -> Result<FilterHandle, &'static str> 
where
    TSourceType: 'static + SceneMessage,
    TTargetType: 'static + SceneMessage,
{
    let mut filters_for_type = (*FILTERS_FOR_TYPE).lock().unwrap();

    // The message type is the key for retrieving this filter later on
    let message_type = (TypeId::of::<TSourceType>(), TypeId::of::<TTargetType>());

    if let Some(filter) = filters_for_type.get(&message_type) {
        // Use the existing filter
        Ok(*filter)
    } else {
        // Create a new filter
        let typed_serializer = (*TYPED_SERIALIZERS).read().unwrap().get(&(TypeId::of::<TSourceType>(), TypeId::of::<TTargetType>())).cloned();
        let typed_serializer = if let Some(typed_serializer) = typed_serializer { Ok(typed_serializer) } else { Err("The requested serializers are not installed") }?;
        let typed_serializer = if let Ok(typed_serializer) = typed_serializer.downcast::<Box<dyn Send + Sync + Fn(TSourceType) -> Result<TTargetType, TSourceType>>>() { 
            Ok(typed_serializer)
        } else {
            Err("Could not properly resolve the type of the requested serializer")
        }?;

        // Create a filter that uses the stored serializer
        let filter_handle = FilterHandle::for_filter(move |input_messages| {
            let typed_serializer = Arc::clone(&typed_serializer);

            input_messages.flat_map(move |msg| stream::iter((*typed_serializer)(msg).ok()))
        });

        // Store for future use
        filters_for_type.insert(message_type, filter_handle);

        // Result is the new filter
        Ok(filter_handle)
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
    pub fn with_serializer<TSerializer>(&self, create_serializer: impl 'static + Send + Sync + Fn() -> TSerializer) -> SceneWithSerializer<'_, TSerializer> 
    where
        TSerializer:                    'static + Send + Serializer,
        TSerializer::Ok:                'static + Send + Unpin,
        for <'b> &'b TSerializer::Ok:   Deserializer<'b>,
    {
        install_serializer(create_serializer);

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

impl<'a, TSerializer> SceneWithSerializer<'a, TSerializer> 
where
    TSerializer:                    'static + Send + Serializer,
    TSerializer::Ok:                'static + Send + Unpin,
    for <'b> &'b TSerializer::Ok:   Deserializer<'b>,
{
    ///
    /// Adds filters to support serializing and deserializing the specified message type
    ///
    /// The name passed in here must be unique for the message type, or an error will be produced
    ///
    pub fn with_serializable_type<TMessageType>(self, type_name: impl Into<String>) -> Self
    where
        TMessageType:  'static + SceneMessage,
        TMessageType:  for<'c> Deserialize<'c>,
        TMessageType:  Serialize,
    {
        // Install the serializers for this type if they aren't already
        install_serializable_type::<TMessageType, TSerializer>(type_name).unwrap();

        // Create filters
        let serialize_filter    = serializer_filter::<TMessageType, SerializedMessage<TSerializer::Ok>>().unwrap();
        let deserialize_filter  = serializer_filter::<SerializedMessage<TSerializer::Ok>, TMessageType>().unwrap();

        self.0.connect_programs(StreamSource::Filtered(serialize_filter), (), StreamId::with_message_type::<TMessageType>()).ok();
        self.0.connect_programs(StreamSource::Filtered(deserialize_filter), (), StreamId::with_message_type::<SerializedMessage<TSerializer::Ok>>()).ok();

        self
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
}

use crate::error::*;
use crate::filter::*;
use crate::input_stream::*;
use crate::output_sink::*;
use crate::scene::*;
use crate::scene_core::*;
use crate::scene_message::*;
use crate::serialization::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::subprogram_id::*;

use futures::task::{Waker};
use once_cell::sync::{Lazy};

use std::any::*;
use std::collections::*;
use std::hash::*;
use std::sync::*;

static STREAM_TYPE_FUNCTIONS: Lazy<RwLock<HashMap<TypeId, StreamTypeFunctions>>> = Lazy::new(|| RwLock::new(HashMap::new()));

type ConnectOutputToInputFn     = Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>, &Arc<dyn Send + Sync + Any>, bool) -> Result<Option<Waker>, ConnectionError>>;
type ConnectOutputToDiscardFn   = Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<Option<Waker>, ConnectionError>>;
type DisconnectOutputFn         = Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<Option<Waker>, ConnectionError>>;
type CloseInputFn               = Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<Option<Waker>, ConnectionError>>;
type IsIdleFn                   = Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<bool, ConnectionError>>;
type WaitingForIdleFn           = Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>, usize) -> Result<IdleInputStreamCore, ConnectionError>>;
type DefaultTargetFn            = Arc<dyn Send + Sync + Fn() -> StreamTarget>;
type ActiveTargetFn             = Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<StreamTarget, ConnectionError>>;
type ReconnectSinkFn            = Arc<dyn Send + Sync + Fn(&Arc<Mutex<SceneCore>>, &Arc<dyn Send + Sync + Any>, SubProgramId, StreamTarget) -> Result<Option<Waker>, ConnectionError>>;
type InitialiseFn               = Arc<dyn Send + Sync + Fn(&Scene)>;

///
/// Functions that work on the 'Any' versions of various streams, used for creating connections
///
struct StreamTypeFunctions {
    /// Connects an OutputSinkCore to a InputStreamCore
    connect_output_to_input: ConnectOutputToInputFn,

    /// Connects an OutputSinkCore to a stream that discards everything
    connect_output_to_discard: ConnectOutputToDiscardFn,

    /// Disconnects an OutputSinkCore, causing it to wait for a new connection to be made
    disconnect_output: DisconnectOutputFn,

    /// Closes the input to a stream
    close_input: CloseInputFn,

    /// Indicates if an input stream is idle or not (idle = has an empty input queue and is waiting for a new message to arrive)
    is_idle: IsIdleFn,

    /// Indicates that the input stream is in a 'waiting for idle' state (where it will queue messages up to a limit until the scene is idle)
    waiting_for_idle: WaitingForIdleFn,

    /// Returns the default target for this stream type
    default_target: DefaultTargetFn,

    /// Returns the active target for an output sink
    active_target: ActiveTargetFn,

    /// Reconnects an output sink core to an input stream
    reconnect_sink: ReconnectSinkFn,

    /// Initialises the message type inside a scene
    initialise: InitialiseFn,
}

///
/// Identifies a stream produced by a subprogram 
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum StreamIdType {
    /// A stream identified by its message type
    MessageType,

    /// A stream sending data to a specific target
    Target(StreamTarget),
}

///
/// Identifies a stream produced by a subprogram 
///
#[derive(Clone, Eq, Debug)]
pub struct StreamId {
    label:                  Option<String>,
    stream_id_type:         StreamIdType,
    message_type_name:      &'static str,
    message_type:           TypeId,
    input_stream_core_type: TypeId,
}

impl PartialEq for StreamId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.stream_id_type == other.stream_id_type && self.message_type == other.message_type
    }
}

impl Hash for StreamId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.stream_id_type.hash(state);
        self.message_type.hash(state);
    }
}

impl StreamTypeFunctions {
    ///
    /// Creates the stream type functions for a particular message type
    ///
    pub fn for_message_type<TMessageType>() -> Self 
    where
        TMessageType: 'static + SceneMessage,
    {
        // Maps types to their filter handles (so we only create the filters once per application)
        // Could be simplified if Rust ever adds support for generic static values
        static FILTERS: Lazy<RwLock<HashMap<TypeId, Vec<FilterHandle>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

        StreamTypeFunctions {
            connect_output_to_input: Arc::new(|output_sink_any, input_stream_any, close_when_dropped| {
                // Cast the 'any' stream and sink to the appropriate types
                let output_sink     = output_sink_any.clone().downcast::<Mutex<OutputSinkCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;
                let input_stream    = input_stream_any.clone().downcast::<Mutex<InputStreamCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;

                // Connect the input stream core to the output target
                let waker = {
                    let mut output_sink = output_sink.lock().unwrap();

                    output_sink.target  = if !close_when_dropped {
                        OutputSinkTarget::Input(Arc::downgrade(&input_stream))
                    } else {
                        OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&input_stream))
                    };

                    output_sink.when_target_changed.take()
                };

                Ok(waker)
            }),

            connect_output_to_discard: Arc::new(|output_sink_any| {
                // Cast the output sink to the appropriate type and set it as discarding any input
                let output_sink = output_sink_any.clone().downcast::<Mutex<OutputSinkCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;

                let waker = {
                    let mut output_sink = output_sink.lock().unwrap();

                    output_sink.target = OutputSinkTarget::Discard;
                    output_sink.when_target_changed.take()
                };

                Ok(waker)
            }),

            disconnect_output: Arc::new(|output_sink_any| {
                // Cast the output sink to the appropriate type and set it as disconnected
                let output_sink = output_sink_any.clone().downcast::<Mutex<OutputSinkCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;

                let waker = {
                    let mut output_sink = output_sink.lock().unwrap();

                    output_sink.target = OutputSinkTarget::Disconnected;
                    output_sink.when_target_changed.take()
                };

                Ok(waker)
            }),

            close_input: Arc::new(|input_stream_any| {
                let input_stream    = input_stream_any.clone().downcast::<Mutex<InputStreamCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;
                let waker           = input_stream.lock().unwrap().close();

                Ok(waker)
            }),

            is_idle: Arc::new(|input_stream_any| {
                let input_stream    = input_stream_any.clone().downcast::<Mutex<InputStreamCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;
                let is_idle         = input_stream.lock().unwrap().is_idle();

                Ok(is_idle)
            }),

            waiting_for_idle: Arc::new(|input_stream_any, max_idle_queue_len| {
                let input_stream    = input_stream_any.clone().downcast::<Mutex<InputStreamCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;
                let dropper         = InputStreamCore::<TMessageType>::waiting_for_idle(&input_stream, max_idle_queue_len);

                Ok(dropper)
            }),

            default_target: Arc::new(|| {
                TMessageType::default_target()
            }),

            active_target: Arc::new(|output_sink_core_any| {
                let output_sink         = output_sink_core_any.clone().downcast::<Mutex<OutputSinkCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;
                let output_sink_target  = output_sink.lock().unwrap().target.clone();

                match &output_sink_target {
                    OutputSinkTarget::Disconnected                  => Ok(StreamTarget::Any),
                    OutputSinkTarget::Discard                       => Ok(StreamTarget::None),
                    OutputSinkTarget::Input(input_core)             |
                    OutputSinkTarget::CloseWhenDropped(input_core)  => {
                        if let Some(input_core) = input_core.upgrade() {
                            // Target is the program being run by the input stream core
                            // TODO: properly figure out filters (this will be the 'fake' input program for a filter if a filter is in use)
                            Ok(StreamTarget::Program(input_core.lock().unwrap().target_program_id()))
                        } else {
                            // Input core has been lost (next message will generate an error, we'll indicate 'None' as the connection)
                            Ok(StreamTarget::None)
                        }
                    }
                }
            }),

            reconnect_sink: Arc::new(|scene_core, output_sink_core_any, source_program, stream_target| {
                // Try to create an output sink target for this message type
                let new_target = SceneCore::sink_for_target::<TMessageType>(scene_core, &source_program, stream_target)?;

                // Update the output sink
                let output_sink = output_sink_core_any.clone().downcast::<Mutex<OutputSinkCore<TMessageType>>>().map_err(|_| ConnectionError::UnexpectedConnectionType)?;

                let waker = {
                    let mut output_sink = output_sink.lock().unwrap();

                    output_sink.target = new_target;

                    output_sink.when_target_changed.take()
                };

                Ok(waker)
            }),

            initialise: Arc::new(move |scene| {
                use std::mem;

                let serialization_filters = {
                    let filters = (*FILTERS).read().unwrap();
                    if let Some(existing_filters) = filters.get(&TypeId::of::<TMessageType>()) {
                        // We only create the filters once
                        existing_filters.clone()
                    } else {
                        mem::drop(filters);

                        // Set up the serialization for this type if it's not already set up
                        #[cfg(feature="serde_json")]
                        install_serializable_type::<TMessageType, serde_json::Value>().unwrap();

                        // Create the filters for this type
                        let mut filters = (*FILTERS).write().unwrap();
                        if let Some(existing_filters) = filters.get(&TypeId::of::<TMessageType>()) {
                            // Lost the race: someone else created the filters
                            existing_filters.clone()
                        } else {
                            // Create the filters for this type
                            let new_filters = TMessageType::create_serializer_filters();
                            filters.insert(TypeId::of::<TMessageType>(), new_filters.clone());

                            new_filters
                        }
                    }
                };

                // Install the default filters for this type
                for filter in serialization_filters.iter() {
                    scene.connect_programs(StreamSource::Filtered(*filter), (), filter.source_stream_id_any().unwrap().with_target_type_name(TMessageType::message_type_name())).ok();
                }

                // Call the message-specific initialisation
                TMessageType::initialise(scene)
            }),
        }
    }

    ///
    /// Store the type functions for a message type, if they aren't stored already
    ///
    pub fn add<TMessageType>()
    where
        TMessageType: 'static + SceneMessage,
    {
        let type_id                     = TypeId::of::<TMessageType>();
        let mut stream_type_functions   = STREAM_TYPE_FUNCTIONS.write().unwrap();

        stream_type_functions.entry(type_id)
            .or_insert_with(|| StreamTypeFunctions::for_message_type::<TMessageType>());
    }

    ///
    /// Retrieves the 'connect input to output' function for a particular type ID, if it exists
    ///
    pub fn connect_output_to_input(type_id: &TypeId) -> Option<ConnectOutputToInputFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.connect_output_to_input))
    }


    pub fn connect_output_to_discard(type_id: &TypeId) -> Option<ConnectOutputToDiscardFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.connect_output_to_discard))
    }

    pub fn disconnect_output(type_id: &TypeId) -> Option<DisconnectOutputFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.disconnect_output))
    }

    pub fn close_input(type_id: &TypeId) -> Option<CloseInputFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.close_input))
    }

    pub fn is_idle(type_id: &TypeId) -> Option<IsIdleFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.is_idle))
    }

    pub fn waiting_for_idle(type_id: &TypeId) -> Option<WaitingForIdleFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.waiting_for_idle))
    }

    pub fn default_target(type_id: &TypeId) -> Option<DefaultTargetFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.default_target))
    }

    pub fn active_target(type_id: &TypeId) -> Option<ActiveTargetFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.active_target))
    }

    pub fn reconnect_output_sink(type_id: &TypeId) -> Option<ReconnectSinkFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.reconnect_sink))
    }

    pub fn initialise(type_id: &TypeId) -> Option<InitialiseFn> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(type_id)
            .map(|all_functions| Arc::clone(&all_functions.initialise))
    }
}

impl StreamId {
    ///
    /// ID of a stream that generates a particular type of data
    ///
    pub fn with_message_type<TMessageType>() -> Self 
    where
        TMessageType: 'static + SceneMessage,
    {
        StreamTypeFunctions::add::<TMessageType>();

        StreamId {
            label:                  None,
            stream_id_type:         StreamIdType::MessageType,
            message_type_name:      type_name::<TMessageType>(),
            message_type:           TypeId::of::<TMessageType>(),
            input_stream_core_type: TypeId::of::<Mutex<InputStreamCore<TMessageType>>>(),
        }
    }

    ///
    /// ID of a stream that is assigned to a particular target
    ///
    pub fn for_target(&self, target: impl Into<StreamTarget>) -> Self {
        StreamId {
            label:                  None,
            stream_id_type:         StreamIdType::Target(target.into()),
            message_type_name:      self.message_type_name,
            message_type:           self.message_type,
            input_stream_core_type: self.input_stream_core_type,
        }
    }

    ///
    /// Returns a stream ID with a target type name attached to it.
    ///
    /// This is used for serialized messages where the message type is a generic serialized message (such as
    /// `SerializedMessage<serde_json::Value>`) but the type can be deserialized as one of many other types:
    /// this is used to determine the 'true' target of a stream of serialized messages.
    ///
    pub fn with_target_type_name(&self, message_type_name: impl Into<String>) -> Self {
        StreamId {
            label:                  Some(message_type_name.into()),
            stream_id_type:         self.stream_id_type.clone(),
            message_type_name:      self.message_type_name,
            message_type:           self.message_type,
            input_stream_core_type: self.input_stream_core_type,
        }
    }

    ///
    /// Returns a stream ID that has no target program but is otherwise the same as the current stream
    ///
    pub fn as_message_type(&self) -> Self {
        StreamId {
            label:                  None,
            stream_id_type:         StreamIdType::MessageType,
            message_type_name:      self.message_type_name,
            message_type:           self.message_type,
            input_stream_core_type: self.input_stream_core_type,
        }
    }

    ///
    /// None if this stream ID is not for a specific target, otherwise the program ID of the target that this stream is for
    ///
    pub fn target_program(&self) -> Option<SubProgramId> {
        match self.stream_id_type {
            StreamIdType::MessageType                                   => None,
            StreamIdType::Target(StreamTarget::Program(target_id))      => Some(target_id),
            StreamIdType::Target(StreamTarget::Filtered(_, target_id))  => Some(target_id),
            StreamIdType::Target(_)                                     => None,
        }
    }

    ///
    /// The type of message that can be sent to this stream
    ///
    pub fn message_type(&self) -> TypeId {
        self.message_type
    }

    ///
    /// The name of the Rust type that is the expected type name for this stream
    ///
    pub fn message_type_name(&self) -> String {
        self.message_type_name.into()
    }

    ///
    /// Returns the default target defined for the message type represented by this stream ID
    ///
    pub fn default_target(&self) -> StreamTarget {
        let message_type = self.message_type();

        if let Some(default_target) = StreamTypeFunctions::default_target(&message_type) {
            default_target()
        } else {
            StreamTarget::None
        }
    }

    ///
    /// The type of the `Mutex<InputStreamCore<...>>` that will be used for the stream id
    ///
    pub (crate) fn input_stream_core_type(&self) -> TypeId {
        self.input_stream_core_type
    }

    ///
    /// Given an output sink (an 'Any' that maps to an OutputSinkCore) and an input stream (an 'Any' that maps to an InputStreamCore) that match
    /// the type of this stream ID, sends the data from the output sink to the input stream.
    ///
    /// Note that this locks the output target.
    ///
    pub (crate) fn connect_output_to_input(&self, output_sink: &Arc<dyn Send + Sync + Any>, input_stream: &Arc<dyn Send + Sync + Any>, close_when_dropped: bool) -> Result<Option<Waker>, ConnectionError> {
        let message_type = self.message_type();

        if let Some(connect_input) = StreamTypeFunctions::connect_output_to_input(&message_type) {
            // Connect the input to the output
            (connect_input)(output_sink, input_stream, close_when_dropped)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }

    ///
    /// Given an output sink (an 'Any' that maps to an OutputSinkCore), connects it to a stream that just throws any messages it receives away
    ///
    /// Note that this locks the output target.
    ///
    pub (crate) fn connect_output_to_discard(&self, output_sink: &Arc<dyn Send + Sync + Any>) -> Result<Option<Waker>, ConnectionError> {
        let message_type = self.message_type();

        if let Some(connect_input) = StreamTypeFunctions::connect_output_to_discard(&message_type) {
            // Connect the input to the output
            (connect_input)(output_sink)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }

    ///
    /// Given an output sink (an 'Any' that maps to an OutputSinkCore of the same type as this stream ID), disconnects it so it waits for a new connection
    ///
    /// Note that this locks the output target.
    ///
    pub (crate) fn disconnect_output(&self, output_sink: &Arc<dyn Send + Sync + Any>) -> Result<Option<Waker>, ConnectionError> {
        let message_type = self.message_type();

        if let Some(connect_input) = StreamTypeFunctions::disconnect_output(&message_type) {
            // Disconnect the output sink
            (connect_input)(output_sink)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }

    ///
    /// Closes an input stream (an 'Any' that maps to an InputStreamCore of the same type as this stream ID) 
    ///
    pub (crate) fn close_input(&self, input_stream: &Arc<dyn Send + Sync + Any>) -> Result<Option<Waker>, ConnectionError> {
        let message_type = self.message_type();

        if let Some(close_input) = StreamTypeFunctions::close_input(&message_type) {
            // Close the input stream
            (close_input)(input_stream)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }

    ///
    /// Given an input stream (an 'Any' that maps to an InputStreamCore of the same type as this stream ID), returns
    /// whether or not it is considered to be idle (being waited upon + has an empty queue)
    ///
    pub (crate) fn is_idle(&self, input_stream: &Arc<dyn Send + Sync  + Any>) -> Result<bool, ConnectionError> {
        let message_type = self.message_type();

        if let Some(is_idle) = StreamTypeFunctions::is_idle(&message_type) {
            // Determine if the stream is idle
            (is_idle)(input_stream)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }

    ///
    /// Given an input stream, indicates that's in the 'waiting for idle' state with the specified length of allowed extra waiting messages
    ///
    pub (crate) fn waiting_for_idle(&self, input_stream: &Arc<dyn Send + Sync  + Any>, max_idle_queue_len: usize) -> Result<IdleInputStreamCore, ConnectionError> {
        let message_type = self.message_type();

        if let Some(waiting_for_idle) = StreamTypeFunctions::waiting_for_idle(&message_type) {
            // Determine if the stream is idle
            (waiting_for_idle)(input_stream, max_idle_queue_len)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }

    ///
    /// Calls the 'initialise' function for a message type within a scene
    ///
    /// (Each type should only be initialised once per scene)
    ///
    pub (crate) fn initialise_in_scene(&self, scene: &Scene) -> Result<(), ConnectionError> {
        let message_type = self.message_type();

        if let Some(initialise) = StreamTypeFunctions::initialise(&message_type) {
            // Determine if the stream is idle
            (initialise)(scene);
            Ok(())
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }

    ///
    /// Returns the stream target for an output sink
    ///
    pub (crate) fn active_target_for_output_sink(&self, output_sink_core: &Arc<dyn Send + Sync + Any>) -> Result<StreamTarget, ConnectionError> {
        let message_type = self.message_type();

        if let Some(active_target) = StreamTypeFunctions::active_target(&message_type) {
            (active_target)(output_sink_core)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }

    ///
    /// Attempts to reconnect an output sink core to a new target within a scene (returning a waker if successful)
    ///
    pub (crate) fn reconnect_output_sink(&self, scene_core: &Arc<Mutex<SceneCore>>, output_sink_core: &Arc<dyn Send + Sync + Any>, source_program: SubProgramId, new_target: StreamTarget) -> Result<Option<Waker>, ConnectionError> {
        let message_type = self.message_type();

        if let Some(reconnect_output_sink) = StreamTypeFunctions::reconnect_output_sink(&message_type) {
            (reconnect_output_sink)(scene_core, output_sink_core, source_program, new_target)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(ConnectionError::UnexpectedConnectionType)
        }
    }
}

mod serialization {
    use super::*;

    use serde::*;

    #[derive(Serialize, Deserialize)]
    enum SerializedStreamId {
        /// A known serializable type
        Serializable { type_name: String, target: Option<SubProgramId> },

        /// A Rust type, with the specified type name (note that this name may not be consistent between applications)
        RustType { type_name: String, target: Option<SubProgramId> },
    }

    impl Serialize for StreamId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let serialized = if let Some(serializable_name) = self.serialization_type_name() {
                SerializedStreamId::Serializable { type_name: serializable_name, target: self.target_program() }
            } else {
                SerializedStreamId::RustType { type_name: self.message_type_name(), target: self.target_program() }
            };

            serialized.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for StreamId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let stream_id = SerializedStreamId::deserialize(deserializer)?;

            match stream_id {
                SerializedStreamId::Serializable { type_name, target } => {
                    if let Some(stream_id) = StreamId::with_serialization_type(type_name) {
                        if let Some(target) = target {
                            Ok(stream_id.for_target(target))
                        } else {
                            Ok(stream_id)
                        }
                    } else {
                        // TODO: generate an error
                        todo!()
                    }
                }

                SerializedStreamId::RustType { type_name, target } => {
                    // TODO: store, look up this type
                    todo!()
                }
            }
        }
    }
}

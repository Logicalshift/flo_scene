use crate::input_stream::*;
use crate::output_sink::*;
use crate::stream_target::*;

use once_cell::sync::{Lazy};

use std::any::*;
use std::collections::*;
use std::sync::*;

static STREAM_TYPE_FUNCTIONS: Lazy<RwLock<HashMap<TypeId, StreamTypeFunctions>>> = Lazy::new(|| RwLock::new(HashMap::new()));

///
/// Functions that work on the 'Any' versions of various streams, used for creating connections
///
struct StreamTypeFunctions {
    /// Connects an OutputSinkTarget to a InputStreamCore
    connect_output_to_input: Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>, &Arc<dyn Send + Sync + Any>, bool) -> Result<(), ()>>,

    /// Connects an OutputSinkTarget to a stream that discards everything
    connect_output_to_discard: Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<(), ()>>,

    /// Disconnects an OutputSinkTarget, causing it to wait for a new connection to be made
    disconnect_output: Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<(), ()>>,
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
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct StreamId {
    stream_id_type:         StreamIdType,
    message_type_name:      &'static str,
    message_type:           TypeId,
    input_stream_core_type: TypeId,
}

impl StreamTypeFunctions {
    ///
    /// Creates the stream type functions for a particular message type
    ///
    pub fn for_message_type<TMessageType>() -> Self 
    where
        TMessageType: 'static + Send + Sync,
    {
        StreamTypeFunctions {
            connect_output_to_input: Arc::new(|output_sink_any, input_stream_any, close_when_dropped| {
                // Cast the 'any' stream and sink to the appropriate types
                let output_sink     = output_sink_any.clone().downcast::<Mutex<OutputSinkTarget<TMessageType>>>().map_err(|_| ())?;
                let input_stream    = input_stream_any.clone().downcast::<Mutex<InputStreamCore<TMessageType>>>().map_err(|_| ())?;

                // Connect the input stream core to the output target
                *output_sink.lock().unwrap() = if !close_when_dropped {
                    OutputSinkTarget::Input(Arc::downgrade(&input_stream))
                } else {
                    OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&input_stream))
                };

                Ok(())
            }),

            connect_output_to_discard: Arc::new(|output_sink_any| {
                // Cast the output sink to the appropriate type and set it as discarding any input
                let output_sink     = output_sink_any.clone().downcast::<Mutex<OutputSinkTarget<TMessageType>>>().map_err(|_| ())?;
                *output_sink.lock().unwrap() = OutputSinkTarget::Discard;

                Ok(())
            }),

            disconnect_output: Arc::new(|output_sink_any| {
                // Cast the output sink to the appropriate type and set it as disconnected
                let output_sink     = output_sink_any.clone().downcast::<Mutex<OutputSinkTarget<TMessageType>>>().map_err(|_| ())?;
                *output_sink.lock().unwrap() = OutputSinkTarget::Disconnected;

                Ok(())
            }),
        }
    }

    ///
    /// Store the type functions for a message type, if they aren't stored already
    ///
    pub fn add<TMessageType>()
    where
        TMessageType: 'static + Send + Sync,
    {
        let type_id                     = TypeId::of::<TMessageType>();
        let mut stream_type_functions   = STREAM_TYPE_FUNCTIONS.write().unwrap();

        stream_type_functions.entry(type_id)
            .or_insert_with(|| StreamTypeFunctions::for_message_type::<TMessageType>());
    }

    ///
    /// Retrieves the 'connect input to output' function for a particular type ID, if it exists
    ///
    pub fn connect_output_to_input(type_id: &TypeId) -> Option<Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>, &Arc<dyn Send + Sync + Any>, bool) -> Result<(), ()>>> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(&type_id)
            .map(|all_functions| Arc::clone(&all_functions.connect_output_to_input))
    }


    pub fn connect_output_to_discard(type_id: &TypeId) -> Option<Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<(), ()>>> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(&type_id)
            .map(|all_functions| Arc::clone(&all_functions.connect_output_to_discard))
    }

    pub fn disconnect_output(type_id: &TypeId) -> Option<Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>) -> Result<(), ()>>> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(&type_id)
            .map(|all_functions| Arc::clone(&all_functions.disconnect_output))
    }
}

impl StreamId {
    ///
    /// ID of a stream that generates a particular type of data
    ///
    pub fn with_message_type<TMessageType>() -> Self 
    where
        TMessageType: 'static + Send + Sync,
    {
        StreamTypeFunctions::add::<TMessageType>();

        StreamId {
            stream_id_type:         StreamIdType::MessageType,
            message_type_name:      type_name::<TMessageType>(),
            message_type:           TypeId::of::<TMessageType>(),
            input_stream_core_type: TypeId::of::<Mutex<InputStreamCore<TMessageType>>>(),
        }
    }

    ///
    /// ID of a stream that is assigned to a particular target
    ///
    pub fn for_target<TMessageType>(target: impl Into<StreamTarget>) -> Self
    where
        TMessageType: 'static + Send + Sync,
    {
        StreamTypeFunctions::add::<TMessageType>();

        StreamId {
            stream_id_type:         StreamIdType::Target(target.into()),
            message_type_name:      type_name::<TMessageType>(),
            message_type:           TypeId::of::<TMessageType>(),
            input_stream_core_type: TypeId::of::<Mutex<InputStreamCore<TMessageType>>>(),
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
    /// The type of the `Mutex<InputStreamCore<...>>` that will be used for the stream id
    ///
    pub (crate) fn input_stream_core_type(&self) -> TypeId {
        self.input_stream_core_type
    }

    ///
    /// Given an output sink (an 'Any' that maps to an OutputSinkTarget) and an input stream (an 'Any' that maps to an InputStreamCore) that match
    /// the type of this stream ID, sends the data from the output sink to the input stream.
    ///
    /// Note that this locks the output target.
    ///
    pub (crate) fn connect_output_to_input(&self, output_sink: &Arc<dyn Send + Sync + Any>, input_stream: &Arc<dyn Send + Sync + Any>, close_when_dropped: bool) -> Result<(), ()> {
        let message_type = self.message_type();

        if let Some(connect_input) = StreamTypeFunctions::connect_output_to_input(&message_type) {
            // Connect the input to the output
            (connect_input)(output_sink, input_stream, close_when_dropped)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(())
        }
    }

    ///
    /// Given an output sink (an 'Any' that maps to an OutputSinkTarget), connects it to a stream that just throws any messages it receives away
    ///
    /// Note that this locks the output target.
    ///
    pub (crate) fn connect_output_to_discard(&self, output_sink: &Arc<dyn Send + Sync + Any>) -> Result<(), ()> {
        let message_type = self.message_type();

        if let Some(connect_input) = StreamTypeFunctions::connect_output_to_discard(&message_type) {
            // Connect the input to the output
            (connect_input)(output_sink)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(())
        }
    }

    ///
    /// Given an output sink (an 'Any' that maps to an OutputSinkTarget), disconnects it so it waits for a new connection
    ///
    /// Note that this locks the output target.
    ///
    pub (crate) fn disconnect_output(&self, output_sink: &Arc<dyn Send + Sync + Any>) -> Result<(), ()> {
        let message_type = self.message_type();

        if let Some(connect_input) = StreamTypeFunctions::disconnect_output(&message_type) {
            // Connect the input to the output
            (connect_input)(output_sink)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(())
        }
    }
}

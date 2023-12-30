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
    connect_output_to_input: Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>, &Arc<dyn Send + Sync + Any>) -> Result<(), ()>>,
}

///
/// Identifies a stream produced by a subprogram 
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum StreamId {
    /// A stream identified by its message type
    MessageType(TypeId),

    /// A stream sending data to a specific target
    Target(TypeId, StreamTarget),
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
            connect_output_to_input: Arc::new(|output_sink_any, input_stream_any| {
                // Cast the 'any' stream and sink to the appropriate types
                let output_sink     = output_sink_any.clone().downcast::<Mutex<OutputSinkTarget<TMessageType>>>().map_err(|_| ())?;
                let input_stream    = input_stream_any.clone().downcast::<Mutex<InputStreamCore<TMessageType>>>().map_err(|_| ())?;

                // Connect the input stream core to the output target
                *output_sink.lock().unwrap() = OutputSinkTarget::Input(Arc::downgrade(&input_stream));

                Ok(())
            })
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
    pub fn connect_output_to_input(type_id: &TypeId) -> Option<Arc<dyn Send + Sync + Fn(&Arc<dyn Send + Sync + Any>, &Arc<dyn Send + Sync + Any>) -> Result<(), ()>>> {
        let stream_type_functions = STREAM_TYPE_FUNCTIONS.read().unwrap();

        stream_type_functions.get(&type_id)
            .map(|all_functions| Arc::clone(&all_functions.connect_output_to_input))
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
        StreamId::MessageType(TypeId::of::<TMessageType>())
    }

    ///
    /// ID of a stream that is assigned to a particular target
    ///
    pub fn for_target<TMessageType>(target: impl Into<StreamTarget>) -> Self
    where
        TMessageType: 'static + Send + Sync,
    {
        StreamTypeFunctions::add::<TMessageType>();
        StreamId::Target(TypeId::of::<TMessageType>(), target.into())
    }

    ///
    /// The type of message that can be sent to this stream
    ///
    pub fn message_type(&self) -> TypeId {
        match self {
            StreamId::MessageType(message_type) => *message_type,
            StreamId::Target(message_type, _)   => *message_type,
        }
    }

    ///
    /// Given an output sink (an 'Any' that maps to an OutputSinkTarget) and an input stream (an 'Any' that maps to an InputStreamCore) that match
    /// the type of this stream ID, sends the data from the output sink to the input stream.
    ///
    pub (crate) fn connect_output_to_input(&self, output_sink: &Arc<dyn Send + Sync + Any>, input_stream: &Arc<dyn Send + Sync + Any>) -> Result<(), ()> {
        let message_type = self.message_type();

        if let Some(connect_input) = StreamTypeFunctions::connect_output_to_input(&message_type) {
            // Connect the input to the output
            (connect_input)(output_sink, input_stream)
        } else {
            // Shouldn't happen: the stream type was not registered correctly
            Err(())
        }
    }
}

impl From<TypeId> for StreamId {
    #[inline]
    fn from(type_id: TypeId) -> StreamId {
        StreamId::MessageType(type_id)
    }
}
use super::errors::*;
use super::stream_id::*;
use super::traits::*;

use serde::*;
use futures::prelude::*;

use std::any::*;
use std::fmt::{Debug};
use std::hash::{Hash, Hasher};
use std::sync::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_SERIAL: AtomicUsize = AtomicUsize::new(0);

///
/// Represents a filter from one stream type to another
///
#[derive(Clone)]
pub struct FilterHandle {
    /// Data associated with this filter
    data: Arc<FilterData>,

    /// Serial number that allows us to recognise clones of a filter
    serial: usize,
}

///
/// Internal data for a filter handle
///
struct FilterData {
    /// The type of the items in the input stream
    source_stream_id:   StreamId,

    /// The type of the items in the output stream
    target_stream_id:   StreamId,

    /// Takes a boxed BoxSceneStream, and creates a boxed BoxStream (as Any) by applying this filter
    create_stream:      Box<dyn 'static + Send + Sync + Fn(Box<dyn 'static + Send + Any>) -> Result<Box<dyn 'static + Send + Any>, ConnectionError>>,
}

impl FilterHandle {
    ///
    /// Creates a filter for a filtering function
    ///
    pub fn for_filter<TSourceMessage, TTargetStream>(filter_fn: impl 'static + Send + Sync + Fn(BoxSceneStream<TSourceMessage>) -> TTargetStream) -> Self 
    where
        TSourceMessage:         'static + Unpin + SceneMessage,
        TTargetStream:          'static + Send + Stream,
        TTargetStream::Item:    'static + SceneMessage,
    {
        // Assign a serial number to this new filter
        let serial = NEXT_SERIAL.fetch_add(1, Ordering::Relaxed);

        // Source & target IDs
        let source_stream_id = StreamId::with_message_type::<TSourceMessage>();
        let target_stream_id = StreamId::with_message_type::<TTargetStream::Item>();

        // Create the factory function
        let create_stream = move |boxed_stream: Box<dyn 'static + Send + Any>| {
            // Turn the input stream back into a stream of the appropriate type
            let Ok(input_stream)    = boxed_stream.downcast::<BoxSceneStream<TSourceMessage>>() else { return Err(ConnectionError::FilterInputDoesNotMatch) };
            let input_stream        = *input_stream;

            // Apply the filter function to generate the output
            let output_stream                                   = filter_fn(input_stream).boxed();
            let output_stream: Box<dyn 'static + Send + Any>    = Box::new(output_stream);

            Ok(output_stream)
        };
        let create_stream = Box::new(create_stream);

        // Put into a data structure
        let data = FilterData {
            source_stream_id, target_stream_id, create_stream,
        };

        Self {
            data:   Arc::new(data),
            serial: serial,
        }
    }

    ///
    /// Returns the type of message that this filter reads from
    ///
    #[inline]
    pub fn source_stream_id(&self) -> StreamId {
        self.data.source_stream_id.clone()
    }

    ///
    /// Returns the type of message that this filter writes
    ///
    #[inline]
    pub fn target_stream_id(&self) -> StreamId {
        self.data.target_stream_id.clone()
    }

    ///
    /// Applies this filter to a source stream, returning a target stream
    ///
    pub fn filter<TSourceMessage, TTargetMessage>(&self, source: impl 'static + SceneStream<Item=TSourceMessage>) -> Result<BoxSceneStream<TTargetMessage>, ConnectionError>
    where 
        TSourceMessage: SceneMessage,
        TTargetMessage: SceneMessage,
    {
        let filter_any = self.filter_any::<TSourceMessage>(source)?;
        let Ok(filter) = filter_any.downcast() else { return Err(ConnectionError::FilterOutputDoesNotMatch); };

        Ok(*filter)
    }

    ///
    /// Applies this filter to a source stream, returning an 'Any' that maps to a BoxSceneStream of the target type
    ///
    pub fn filter_any<TSourceMessage>(&self, source: impl 'static + SceneStream<Item=TSourceMessage>) -> Result<Box<dyn 'static + Send + Any>, ConnectionError>
    where 
        TSourceMessage: SceneMessage,
    {
        let source = Box::new(source.into_box());
        (self.data.create_stream)(source)
    }
}

impl PartialEq for FilterHandle {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
    }
}

impl Eq for FilterHandle {
}

impl Hash for FilterHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.serial.hash(state)
    }
}

impl Serialize for FilterHandle {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        use serde::ser::{Error};
        Err(S::Error::custom("Filters cannot be serialized"))
    }
}

impl<'de> Deserialize<'de> for FilterHandle {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de> 
    {
        use serde::de::{Error};
        Err(D::Error::custom("Filters cannot be deserialized"))
    }
}

impl Debug for FilterHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source_stream_id = self.data.source_stream_id.clone();
        let target_stream_id = self.data.target_stream_id.clone();

        write!(f, "FilterHandle({}: {} -> {})", self.serial, source_stream_id.message_type_name(), target_stream_id.message_type_name())
    }
}

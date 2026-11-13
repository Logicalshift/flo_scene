use crate::errors::*;
use super::filter_chain_stream::*;
use super::stream_id::*;
use super::traits::*;

use serde::*;
use futures::prelude::*;
use futures::stream::{BoxStream};

use alloc::sync::*;
use core::any::*;
use core::fmt;
use core::fmt::{Debug};
use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicUsize, Ordering};

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

    /// Creates a filter chain for this filter (takes the 'self' filter handle and a BoxSceneStream input as Any, returning a BoxSceneStream wrapped in an Any)
    create_chain:       Box<dyn 'static + Send + Sync + Fn(&FilterHandle, Box<dyn 'static + Send + Any>) -> Result<Box<dyn 'static + Send + Any>, ConnectionError>>,
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

        // Create the chaining function
        let create_chain = move |our_filter_handle: &FilterHandle, source_stream: Box<dyn 'static + Send + Any>| {
            // Fetch the source stream
            let Ok(source_stream)   = source_stream.downcast::<BoxSceneStream<TSourceMessage>>() else { return Err(ConnectionError::FilterInputDoesNotMatch) };
            let source_stream       = *source_stream;

            // Create a chain for this stream
            let chain_stream = FilterChainStream::<TSourceMessage, TTargetStream::Item>::new(our_filter_handle.clone(), source_stream)?;
            let chain_stream = chain_stream.into_box();
            let chain_stream = Box::new(chain_stream);

            Ok(Box::<dyn 'static + Send + Any>::from(chain_stream))
        };
        let create_chain = Box::new(create_chain);

        // Put into a data structure
        let data = FilterData {
            source_stream_id, target_stream_id, create_stream, create_chain,
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
    pub fn filter<TSourceMessage, TTargetMessage>(&self, source: impl 'static + SceneStream<Item=TSourceMessage>) -> Result<BoxStream<'static, TTargetMessage>, ConnectionError>
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

    ///
    /// Creates a filter that sends the output of this filter to another filter to generate a final result
    ///
    pub fn chain(&self, next_filter: FilterHandle) -> Result<FilterHandle, ConnectionError> {
        if next_filter.source_stream_id().message_type() != self.target_stream_id().message_type() {
            // Next filter must match the last filter
            return Err(ConnectionError::FilterInputDoesNotMatch);
        }

        // Assign a serial number to this new filter
        let serial = NEXT_SERIAL.fetch_add(1, Ordering::Relaxed);

        // Chaining 'create input' function
        let our_filter      = self.clone();
        let their_filter    = next_filter.clone();

        let create_stream = move |source_stream| {
            let our_output      = (our_filter.data.create_chain)(&our_filter, source_stream)?;
            let their_output    = (their_filter.data.create_stream)(our_output)?;

            Ok(their_output)
        };

        // The chained chain function
        let our_filter      = self.clone();
        let their_filter    = next_filter.clone();

        let create_chain = move |_chained_filter_handle: &FilterHandle, source_stream| {
            let our_chain   = (our_filter.data.create_chain)(&our_filter, source_stream)?;
            let their_chain = (their_filter.data.create_chain)(&their_filter, our_chain)?;

            Ok(their_chain)
        };

        // Filter data
        let chained_filter_data = FilterData {
            source_stream_id:   self.source_stream_id(),
            target_stream_id:   next_filter.target_stream_id(),
            create_stream:      Box::new(create_stream),
            create_chain:       Box::new(create_chain),
        };

        Ok(FilterHandle {
            serial: serial,
            data:   Arc::new(chained_filter_data)
        })
    }

    ///
    /// Creates a filter that converts between two message types that implements `From`
    ///
    /// This will cache the filter handle for specific message types so this won't allocate additional filters every time it's called
    ///
    pub fn conversion_filter<TSourceMessage, TTargetMessage>() -> FilterHandle
    where
        TSourceMessage: 'static + SceneMessage + Into<TTargetMessage>,
        TTargetMessage: 'static + SceneMessage,
    {
        let new_filter = Self::for_filter(|input| input.map(|source_message: TSourceMessage| source_message.into()));

        new_filter
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source_stream_id = self.data.source_stream_id.clone();
        let target_stream_id = self.data.target_stream_id.clone();

        write!(f, "FilterHandle({}: {} -> {})", self.serial, source_stream_id.message_type_name(), target_stream_id.message_type_name())
    }
}

use super::errors::*;
use super::stream_id::*;
use super::traits::*;

use serde::*;
use futures::prelude::*;

use std::any::*;

///
/// Represents a filter from one stream type to another
///
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FilterHandle {

}

impl FilterHandle {
    ///
    /// Creates a filter for a filtering function
    ///
    pub fn for_filter<TSourceMessage, TTargetStream>(filter_fn: impl 'static + Send + Fn(BoxSceneStream<TSourceMessage>) -> TTargetStream) -> Self 
    where
        TSourceMessage:         'static + Unpin + SceneMessage,
        TTargetStream:          'static + Send + Stream,
        TTargetStream::Item:    'static + SceneMessage,
    {
        todo!()
    }

    ///
    /// Returns the type of message that this filter reads from
    ///
    pub fn source_stream_id(&self) -> StreamId {
        todo!()
    }

    ///
    /// Returns the type of message that this filter writes
    ///
    pub fn target_stream_id(&self) -> StreamId {
        todo!()
    }

    ///
    /// Applies this filter to a source stream, returning a target stream
    ///
    pub fn filter<TSourceMessage, TTargetMessage>(&self, source: impl 'static + SceneStream<Item=TSourceMessage>) -> Result<BoxSceneStream<TTargetMessage>, ConnectionError>
    where 
        TSourceMessage: SceneMessage,
        TTargetMessage: SceneMessage,
    {
        todo!()
    }

    ///
    /// Applies this filter to a source stream, returning an 'Any' that maps to a BoxSceneStream of the target type
    ///
    pub fn filter_any<TSourceMessage>(&self, source: impl 'static + SceneStream<Item=TSourceMessage>) -> Result<Box<dyn 'static + Send + Any>, ConnectionError>
    where 
        TSourceMessage: SceneMessage,
    {
        todo!()
    }
}

use crate::filter::*;
use crate::scene_message::*;
use crate::serialization::*;
use crate::stream_target::*;

use futures::prelude::*;
use futures::stream;
use futures::stream::{BoxStream};

use serde::*;
use serde::de::{Error as DeError};
use serde::ser::{Error as SeError};

use std::marker::{PhantomData};
use std::pin::*;
use std::sync::*;
use std::task::{Context, Poll};

///
/// A query request is a type of message representing a request for a query response of a particular type
///
pub trait QueryRequest : SceneMessage {
    /// An object receiving this query request will send back a `QueryResponse<Self::ResponseData>`
    type ResponseData: Send + Unpin;

    /// Updates this request to use a different target
    fn with_new_target(self, new_target: StreamTarget) -> Self;
}

///
/// A query is a request to send a single `QueryResponse<TResponseData>` back to its sender.
///
/// Queries are typically identified by their data type. The `Query` message is a bit like the `Subscribe` message
/// except that `Subscribe` creates an ongoing series of messages as events happen, and `Query` returns a stream
/// representing the state at the time that the query was received.
///
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Query<TResponseData: Send + Unpin + SceneMessage>(StreamTarget, PhantomData<TResponseData>);

impl<TResponseData: Send + Unpin + SceneMessage> QueryRequest for Query<TResponseData> {
    type ResponseData = TResponseData;

    #[inline]
    fn with_new_target(mut self, new_target: StreamTarget) -> Self {
        self.0 = new_target;
        self
    }
}

///
/// A query response is the message sent whenever a subprogram accepts a `Query`
///
/// Responses to queries are always streams of data items, and each query message should produce exactly one QueryResponse.
///
pub struct QueryResponse<TResponseData>(BoxStream<'static, TResponseData>);

impl<TResponseData: Send + Unpin + SceneMessage> SceneMessage for Query<TResponseData> {
    #[inline]
    fn message_type_name() -> String { format!("query::{}", TResponseData::message_type_name()) }
}

impl<TResponseData: 'static + Send + SceneMessage> SceneMessage for QueryResponse<TResponseData> {
    fn serializable() -> bool { false }

    fn create_serializer_filters() -> Vec<FilterHandle> {
        use std::iter;

        let filters = iter::empty();

        // Create filters that convert the message type to its serialized equivalent
        #[cfg(feature="serde_json")]
        let filters = {
            // Ensure that TResponseData has serializers set up
            install_serializable_type::<TResponseData, serde_json::Value>().unwrap();

            // Types for serializing and deserializing the response data
            let to_json     = serialization_function::<TResponseData, SerializedMessage<serde_json::Value>>().unwrap();
            let from_json   = serialization_function::<SerializedMessage<serde_json::Value>, TResponseData>().unwrap();

            // Filter to convert the response data to JSON format
            let to_json = FilterHandle::for_filter(move |input_messages| {
                let to_json = Arc::clone(&to_json);

                input_messages.map(move |response: QueryResponse<TResponseData>| {
                    let to_json = Arc::clone(&to_json);
                    let responses = response.flat_map(move |msg| stream::iter((*to_json)(msg).ok()));
                    QueryResponse::with_stream(responses.boxed())
                })
            });

            // Filter to convert the response data from JSON format
            let from_json = FilterHandle::for_filter(move |input_messages| {
                let from_json = Arc::clone(&from_json);

                input_messages.map(move |response: QueryResponse<SerializedMessage<serde_json::Value>>| {
                    let from_json = Arc::clone(&from_json);
                    let responses = response.flat_map(move |msg| stream::iter((*from_json)(msg).ok()));
                    QueryResponse::with_stream(responses.boxed())
                })
            });

            filters.chain([to_json, from_json])
        };

        filters.collect()
    }

    #[inline]
    fn message_type_name() -> String { format!("flo_scene::QueryResponse<{}>", std::any::type_name::<TResponseData>()) }
}

impl<TResponseData: Send> Serialize for QueryResponse<TResponseData> {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        Err(S::Error::custom("QueryResponse cannot be serialized"))
    }
}

impl<'a, TResponseData: Send> Deserialize<'a> for QueryResponse<TResponseData> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a> 
    {
        Err(D::Error::custom("QueryResponse cannot be serialized"))
    }
}

impl<TResponseData: Send> Stream for QueryResponse<TResponseData> {
    type Item = TResponseData;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<TResponseData: 'static + Send> QueryResponse<TResponseData> {
    ///
    /// Maps this response to a new type
    ///
    pub fn map_response<TMapTarget: Send>(self, map_fn: impl 'static + Send + Fn(TResponseData) -> TMapTarget) -> QueryResponse<TMapTarget> {
        QueryResponse(self.0.map(map_fn).boxed())
    }
}

impl<TResponseData: 'static + Send + Unpin + SceneMessage> Query<TResponseData> {
    ///
    /// Creates a query message that will send its response to the specified target
    ///
    #[inline]
    pub fn with_target(target: impl Into<StreamTarget>) -> Self {
        Query(target.into(), PhantomData)
    }

    ///
    /// Creates a query message with no target defined (used for `spawn_query` in scene_context)
    ///
    #[inline]
    pub fn with_no_target() -> Self {
        Query(StreamTarget::None, PhantomData)
    }

    ///
    /// Retrieves the place where the query response should be sent
    ///
    #[inline]
    pub fn target(&self) -> StreamTarget {
        self.0.clone()
    }
}

///
/// Creates a 'Query' message that will return a `QueryResponse<TMessageType>` message to the sender
///
#[inline]
pub fn query<TMessageType: 'static + Send + Unpin + SceneMessage>(target: impl Into<StreamTarget>) -> Query<TMessageType> {
    Query::with_target(target.into())
}

impl<TResponseData: 'static + Send + Unpin> QueryResponse<TResponseData> {
    ///
    /// Creates a query response with a stream of data
    ///
    pub fn with_stream(stream: impl 'static + Send + Stream<Item=TResponseData>) -> Self {
        QueryResponse(stream.boxed())
    }

    ///
    /// Creates a query response with a stream of data
    ///
    pub fn with_iterator<TIter>(stream: TIter) -> Self
    where
        TIter:              'static + Send + IntoIterator<Item=TResponseData>,
        TIter::IntoIter:    'static + Send,
    {
        QueryResponse(stream::iter(stream).boxed())
    }

    ///
    /// Creates a query response that sends a single item of data
    ///
    pub fn with_data(item: TResponseData) -> Self {
        use std::iter;
        QueryResponse(stream::iter(iter::once(item)).boxed())
    }

    ///
    /// A response with no values in it
    ///
    pub fn empty() -> Self {
        QueryResponse(stream::empty().boxed())
    }
}

use crate::scene_message::*;
use crate::stream_target::*;

use futures::prelude::*;
use futures::stream;
use futures::stream::{BoxStream};

use serde::*;
use serde::de::{Error as DeError};
use serde::ser::{Error as SeError};

use std::marker::{PhantomData};
use std::pin::*;
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
pub struct Query<TResponseData: Send + Unpin>(StreamTarget, PhantomData<TResponseData>);

impl<TResponseData: Send + Unpin> QueryRequest for Query<TResponseData> {
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

impl<TResponseData: Send + Unpin> SceneMessage for Query<TResponseData> { }

impl<TResponseData: Send> SceneMessage for QueryResponse<TResponseData> {
    fn serializable() -> bool { false }
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

impl<TResponseData: 'static + Send + Unpin> Query<TResponseData> {
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
pub fn query<TMessageType: 'static + Send + Unpin>(target: impl Into<StreamTarget>) -> Query<TMessageType> {
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

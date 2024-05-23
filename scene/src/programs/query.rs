use crate::scene_message::*;

use futures::prelude::*;
use futures::stream;
use futures::stream::{BoxStream};

use std::marker::{PhantomData};
use std::pin::*;
use std::task::{Context, Poll};

///
/// A query is a request to send a single `QueryResponse<TResponseData>` back to its sender.
///
/// Queries are typically identified by their data type. The `Query` message is a bit like the `Subscribe` message
/// except that `Subscribe` creates an ongoing series of messages as events happen, and `Query` returns a stream
/// representing the state at the time that the query was received.
///
pub struct Query<TResponseData: Send + Unpin>(PhantomData<TResponseData>);

///
/// A query response is the message sent whenever a subprogram accepts a `Query`
///
/// Responses to queries are always streams of data items
///
pub struct QueryResponse<TResponseData>(BoxStream<'static, TResponseData>);

impl<TResponseData: Send + Unpin> SceneMessage for Query<TResponseData> { }

impl<TResponseData: Send + Unpin> SceneMessage for QueryResponse<TResponseData> { }

impl<TResponseData: Send + Unpin> Stream for QueryResponse<TResponseData> {
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

impl<TResponseData: 'static + Send + Unpin> Default for Query<TResponseData> {
    fn default() -> Self {
        Query(PhantomData)
    }
}

///
/// Creates a 'Query' message that will return a `QueryResponse<TMessageType>` message to the sender
///
#[inline]
pub fn query<TMessageType: 'static + Send + Unpin>() -> Query<TMessageType> {
    Query::default()
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
}

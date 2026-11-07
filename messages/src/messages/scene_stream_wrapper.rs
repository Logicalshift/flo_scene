use super::subprogram_id::*;
use super::traits::*;

use futures::prelude::*;
use futures::task::{Context, Poll};

use std::pin::*;

///
/// Turns a stream of (SubProgramId, Message) values into a SceneStream
///
pub struct SceneStreamWrapper<TStream> {
    /// The (SubProgramId, TMessage) stream that this is based on
    pub (super) inner_stream: Option<Pin<Box<TStream>>>,
}

impl<TStream, TMessage> Stream for SceneStreamWrapper<TStream>
where 
    TStream: Stream<Item=(SubProgramId, TMessage)>,
{
    type Item = TMessage;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match TStream::poll_next(self.inner_stream.as_mut().unwrap().as_mut(), context) {
            Poll::Ready(Some((_program, msg)))  => Poll::Ready(Some(msg)),
            Poll::Ready(None)                   => Poll::Ready(None),
            Poll::Pending                       => Poll::Pending
        }
    }
}

impl<TStream, TMessage> SceneStream for SceneStreamWrapper<TStream>
where 
    TStream:    'static + Send + Stream<Item=(SubProgramId, TMessage)>,
    TMessage:   SceneMessage,
{
    fn messages_with_sources_boxed(mut self: Pin<Box<Self>>) -> stream::BoxStream<'static, (SubProgramId, Self::Item)> {
        self.inner_stream.take().unwrap()
    }
}

impl<TStream, TMessage> SceneStreamWrapper<TStream>
where
    TStream: 'static + Send + Stream<Item=(SubProgramId, TMessage)>,
{
    ///
    /// Transforms the message via a mapping function
    ///
    pub fn map<TTarget>(mut self, map_fn: impl 'static + Send + Fn(TMessage) -> TTarget) -> SceneStreamWrapper<impl Stream<Item=(SubProgramId, TTarget)>> {
        let inner_stream = self.inner_stream.take().unwrap();
        let inner_stream = inner_stream.map(move |(program, msg)| {
            (program, map_fn(msg))
        });

        SceneStreamWrapper {
            inner_stream: Some(Box::pin(inner_stream))
        }
    }

    ///
    /// Filters out messages that return false from a function
    ///
    pub fn filter(mut self, filter_fn: impl 'static + Send + Fn(&TMessage) -> bool) -> SceneStreamWrapper<impl Stream<Item=(SubProgramId, TMessage)>> {
        let inner_stream = self.inner_stream.take().unwrap();
        let inner_stream = inner_stream.filter(move |(_, msg)| {
            future::ready(filter_fn(msg))
        });

        SceneStreamWrapper {
            inner_stream: Some(Box::pin(inner_stream))
        }
    }

    ///
    /// Transforms the message via a mapping function, removing any items that return None
    ///
    pub fn filter_map<TTarget>(mut self, map_fn: impl 'static + Send + Fn(TMessage) -> Option<TTarget>) -> SceneStreamWrapper<impl Stream<Item=(SubProgramId, TTarget)>> {
        let inner_stream = self.inner_stream.take().unwrap();
        let inner_stream = inner_stream.filter_map(move |(program, msg)| {
            future::ready(map_fn(msg).map(move |tgt| (program, tgt)))
        });

        SceneStreamWrapper {
            inner_stream: Some(Box::pin(inner_stream))
        }
    }

    ///
    /// Transforms the message via a mapping function
    ///
    pub fn flat_map<TTarget>(mut self, map_fn: impl 'static + Send + Fn(TMessage) -> TTarget) -> SceneStreamWrapper<impl Stream<Item=(SubProgramId, TTarget::Item)>>
    where
        TTarget: IntoIterator,
    {
        let inner_stream = self.inner_stream.take().unwrap();
        let inner_stream = inner_stream.flat_map(move |(program, msg)| {
            stream::iter(map_fn(msg).into_iter().map(move |tgt| (program.clone(), tgt)))
        });

        SceneStreamWrapper {
            inner_stream: Some(Box::pin(inner_stream))
        }
    }
}

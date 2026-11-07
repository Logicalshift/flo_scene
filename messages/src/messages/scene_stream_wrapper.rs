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
    inner_stream: Option<Pin<Box<TStream>>>,
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

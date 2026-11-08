use flo_scene_messages::prelude::*;

use futures::prelude::*;
use futures::stream::{BoxStream};
use futures::task::{Context, Poll};

use std::pin::*;

///
/// A single-source SceneStream
///
pub struct SingleSourceStream<TStream> {
    source: SubProgramId,
    stream: Option<Pin<Box<TStream>>>,
}

impl<TStream> SingleSourceStream<TStream>
where 
    TStream:        'static + Send + Stream,
    TStream::Item:  'static + SceneMessage,
{
    pub fn from_stream(stream: TStream, program_id: SubProgramId) -> Self {
        Self {
            source: program_id,
            stream: Some(Box::pin(stream)),
        }
    }
}

impl<TStream> Stream for SingleSourceStream<TStream>
where 
    TStream: Stream,
{
    type Item = TStream::Item;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Some(stream) = self.stream.as_mut() else { return Poll::Ready(None); };

        stream.as_mut().poll_next(cx)
    }
}

impl<TStream> SceneStream for SingleSourceStream<TStream>
where 
    TStream:        'static + Send + Stream,
    TStream::Item:  'static + SceneMessage,
{
    fn messages_with_sources_boxed(mut self: Pin<Box<Self>>) -> BoxStream<'static, (SubProgramId, Self::Item)> {
        let stream      = self.stream.take().unwrap();
        let program_id  = self.source;
        stream.map(move |item| (program_id, item)).boxed()
    }
}

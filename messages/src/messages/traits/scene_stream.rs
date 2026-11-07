use super::scene_message::*;
use crate::messages::subprogram_id::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use std::pin::*;

///
/// Trait implemented by input streams for a flo_scene scene
///
/// These are streams of messages along with their sender which hide the sender information unless specifically requested. Normally
/// a subprogram shouldn't use information about the sender to decide what to do with a message - ie, if a message sends another message
/// in response to a message, it should send it to a requested target and not back to the sender. To enforce this design constraint,
/// flo_scene hides the sender unless it's specifically requested in a stream.
///
pub trait SceneStream : Send + Stream 
where 
    Self::Item: SceneMessage,
{
    ///
    /// Upgrades this stream to return the messages with the source subprogram IDs
    ///
    fn messages_with_sources_boxed(self: Pin<Box<Self>>) -> BoxStream<'static, (SubProgramId, Self::Item)>;
}

///
/// A SceneStream in a box
///
pub type BoxSceneStream<TMessageType> = Pin<Box<dyn 'static + SceneStream<Item=TMessageType>>>;

impl<TMessageType> SceneStream for BoxSceneStream<TMessageType> 
where 
    TMessageType: SceneMessage,
{
    #[inline]
    fn messages_with_sources_boxed(self: Pin<Box<Self>>) -> BoxStream<'static, (SubProgramId, Self::Item)> {
        (*Pin::into_inner(self)).messages_with_sources_boxed()        
    }
}

pub trait SceneStreamExt : Send + Stream {
    ///
    /// Upgrades this stream to return the messages with the source subprogram IDs
    ///
    fn messages_with_sources(self) -> BoxStream<'static, (SubProgramId, Self::Item)> 
    where 
        Self: 'static + Sized;

    ///
    /// Puts this stream into a box
    ///
    fn into_box(self) -> BoxSceneStream<Self::Item>
    where
        Self: 'static + Sized;
}

impl<TSceneStream> SceneStreamExt for TSceneStream
where 
    TSceneStream:       SceneStream,
    TSceneStream::Item: SceneMessage,
{
    #[inline]
    fn messages_with_sources(self) -> BoxStream<'static, (SubProgramId, Self::Item)> 
    where 
        Self: 'static + Sized,
    {
        Box::pin(self).messages_with_sources_boxed()
    }

    #[inline]
    fn into_box(self) -> BoxSceneStream<Self::Item>
    where
        Self: 'static + Sized,
    {
        Box::pin(self)
    }
}

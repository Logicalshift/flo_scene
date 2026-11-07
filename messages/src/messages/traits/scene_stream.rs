use super::scene_message::*;
use crate::messages::subprogram_id::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use std::pin::*;

///
/// Trait implemented by input streams for a flo_scene scene
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

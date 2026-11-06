use super::scene_message::*;
use crate::messages::subprogram_id::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

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
    fn messages_with_sources(self) -> BoxStream<'static, (SubProgramId, Self::Item)>;
}

///
/// A SceneStream in a box
///
pub type BoxSceneStream<TMessageType> = Box<dyn 'static + SceneStream<Item=TMessageType>>;

use crate::error::*;
use crate::entity_id::*;
use crate::message::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

///
/// The scene core represents the state shared between all entities in a scene
///
pub struct SceneCore {

}

impl Default for SceneCore {
    fn default() -> SceneCore {
        SceneCore {

        }
    }
}

impl SceneCore {
    ///
    /// Creates an entity that processes a particular kind of message
    ///
    pub fn create_entity<TMessage, TResponse, TFn, TFnFuture>(&mut self, entity_id: EntityId, runtime: TFn) -> Result<(), CreateEntityError>
    where
        TMessage:   Send,
        TResponse:  Send,
        TFn:        Send + FnOnce(BoxStream<'static, Message<TMessage, TResponse>>) -> TFnFuture,
        TFnFuture:  Future<Output = ()>,
    {
        todo!()
    }
}

use crate::context::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::error::*;

use futures::prelude::*;

use std::sync::*;

///
/// Requests for controlling a scene as a whole
///
pub enum SceneControlRequest {
    /// Requests that the main scene runtime stop (which will stop all the entities in the scene)
    StopScene,

    /// Closes the main input stream for the specified entity (which should cause it to shut down cleanly)
    CloseEntity(EntityId),

    /// Immediately stops running the specified entity (without allowing it to shut down)
    KillEntity(EntityId),

    /// Leaves the specified entity running but 
    SealEntity(EntityId),
}

///
/// Creates a scene control entity 
///
pub fn create_scene_control_entity(entity_id: EntityId, scene_context: &Arc<SceneContext>) -> Result<impl EntityChannel<Message=SceneControlRequest>, CreateEntityError> {
    scene_context.create_entity(entity_id, |context, messages| {
        async move {
            let mut messages = messages;

            while let Some(msg) = messages.next().await {
                match msg {
                    SceneControlRequest::StopScene                  => { context.stop_scene().ok(); }
                    SceneControlRequest::CloseEntity(target_entity) => { context.close_entity(target_entity).ok(); }
                    SceneControlRequest::KillEntity(target_entity)  => { context.kill_entity(target_entity).ok(); }
                    SceneControlRequest::SealEntity(target_entity)  => { context.seal_entity(target_entity).ok(); }
                }
            }
        }
    })
}

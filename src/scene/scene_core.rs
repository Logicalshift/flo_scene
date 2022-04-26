use super::entity::*;

use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::message::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use std::collections::{HashMap};

///
/// The scene core represents the state shared between all entities in a scene
///
pub struct SceneCore {
    /// The entities that are available in this core
    entities: HashMap<EntityId, Entity>,
}

impl Default for SceneCore {
    fn default() -> SceneCore {
        SceneCore {
            entities: HashMap::new(),
        }
    }
}

impl SceneCore {
    ///
    /// Creates an entity that processes a particular kind of message
    ///
    pub fn create_entity<TMessage, TResponse, TFn, TFnFuture>(&mut self, entity_id: EntityId, runtime: TFn) -> Result<(), CreateEntityError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
        TFn:        Send + FnOnce(BoxStream<'static, Message<TMessage, TResponse>>) -> TFnFuture,
        TFnFuture:  Send + Future<Output = ()>,
    {
        // Create the entity representation for this item
        let (channel, receiver) = EntityChannel::new(5);
        let entity              = self.entities.entry(entity_id).or_insert_with(|| Entity::default());

        entity.register_channel(channel)?;

        // Start the future running
        let future              = runtime(receiver.boxed());
        let future              = future.boxed();

        // TODO: Queue a request in the runtime that we will run the entity

        todo!()
    }

    ///
    /// Creates a default behaviour for a particular message type
    ///
    /// This message type will be accepted for all entities in the scene
    ///
    fn create_default<TMessage, TResponse, TFn, TFnFuture>(&mut self, runtime: TFn) -> Result<(), CreateDefaultError>
    where
        TMessage:   Send,
        TResponse:  Send,
        TFn:        Send + FnOnce(BoxStream<'static, (EntityId, Message<TMessage, TResponse>)>) -> TFnFuture,
        TFnFuture:  Send + Future<Output = ()>,
    {
        todo!()
    }
}

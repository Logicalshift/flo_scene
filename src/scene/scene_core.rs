use super::entity_core::*;

use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::message::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::stream::{BoxStream};
use futures::future::{BoxFuture};

use std::collections::{HashMap};

///
/// The scene core represents the state shared between all entities in a scene
///
pub struct SceneCore {
    /// The entities that are available in this core
    pub (super) entities: HashMap<EntityId, EntityCore>,

    /// Futures waiting to run the entities in this scene
    pub (super) waiting_futures: Vec<BoxFuture<'static, ()>>,

    /// Used by the scene that owns this core to request wake-ups (only one scene can be waiting for a wake up at once)
    pub (super) wake_scene: Option<oneshot::Sender<()>>,
}

impl Default for SceneCore {
    fn default() -> SceneCore {
        SceneCore {
            entities:           HashMap::new(),
            waiting_futures:    vec![],
            wake_scene:         None,
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
        TFnFuture:  'static + Send + Future<Output = ()>,
    {
        // Create the entity representation for this item
        let (channel, receiver) = EntityChannel::new(5);
        let entity              = self.entities.entry(entity_id).or_insert_with(|| EntityCore::default());

        entity.register_channel(channel)?;

        // Start the future running
        let future              = runtime(receiver.boxed());
        let future              = future.boxed();

        // Queue a request in the runtime that we will run the entity
        self.waiting_futures.push(future);

        // Wake up the scene so it can schedule this future
        if let Some(wake_scene) = self.wake_scene.take() {
            wake_scene.send(()).ok();
        }

        Ok(())
    }

    ///
    /// Creates a default behaviour for a particular message type
    ///
    /// This message type will be accepted for all entities in the scene
    ///
    fn create_default<TMessage, TResponse, TFn, TFnFuture>(&mut self, runtime: TFn) -> Result<(), CreateDefaultError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
        TFn:        Send + FnOnce(BoxStream<'static, (EntityId, Message<TMessage, TResponse>)>) -> TFnFuture,
        TFnFuture:  Send + Future<Output = ()>,
    {
        todo!()
    }

    ///
    /// Requests that we send messages to a channel for a particular entity
    ///
    fn send_to<TMessage, TResponse>(&mut self, entity_id: EntityId) -> Result<EntityChannel<TMessage, TResponse>, EntityChannelError> 
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send, 
    {
        // Try to retrieve the entity
        let entity = self.entities.get(&entity_id);
        let entity = if let Some(entity) = entity { entity } else { return Err(EntityChannelError::NoSuchEntity); };

        // Attach to the channel in the entity that belongs to this stream type
        // TODO: attach to a default channel if the entity doesn't have this channel
        // TODO: default channels need to know how to upgrade to the 'real' channel if one is created
        let channel = entity.attach_channel();
        let channel = if let Some(channel) = channel { channel } else { return Err(EntityChannelError::NotListening); };

        Ok(channel)
    }
}

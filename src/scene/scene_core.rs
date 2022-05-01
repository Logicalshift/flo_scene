use super::entity_core::*;

use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::message::*;
use crate::context::*;

use ::desync::scheduler::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::stream::{BoxStream};
use futures::future;
use futures::future::{BoxFuture};

use std::sync::*;
use std::collections::{HashMap};

///
/// The scene core represents the state shared between all entities in a scene
///
pub struct SceneCore {
    /// The entities that are available in this core
    pub (super) entities: HashMap<EntityId, Arc<Mutex<EntityCore>>>,

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
    pub (crate) fn create_entity<TMessage, TResponse, TFn, TFnFuture>(&mut self, scene_context: Arc<SceneContext>, runtime: TFn) -> Result<(), CreateEntityError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
        TFn:        'static + Send + FnOnce(BoxStream<'static, Message<TMessage, TResponse>>) -> TFnFuture,
        TFnFuture:  'static + Send + Future<Output = ()>,
    {
        // The entity ID is specified in the supplied scene context
        let entity_id           = scene_context.entity().unwrap();

        // The entity must not already exist
        if self.entities.contains_key(&entity_id) { return Err(CreateEntityError::AlreadyExists); }

        // Create the channel and the eneity
        let (channel, receiver) = EntityChannel::new(5);
        let entity              = Arc::new(Mutex::new(EntityCore::new(channel)));
        let queue               = entity.lock().unwrap().queue();

        self.entities.insert(entity_id, entity);

        // Start the future running
        let future              = async move {
            let future = scheduler().future_desync(&queue, move || async move {
                // Start the future running
                let receiver            = receiver.boxed();
                let mut runtime_future  = SceneContext::with_context(&scene_context, || runtime(receiver).boxed()).unwrap();

                // Poll it in the scene context
                future::poll_fn(|ctxt| {
                    SceneContext::with_context(&scene_context, || 
                        runtime_future.poll_unpin(ctxt)).unwrap()
                }).await;

                // Return the context once we're done
                scene_context
            }.boxed());

            // Run the future, and retrieve the scene context
            let scene_context = future.await.ok();

            // When done, deregister the entity
            if let Some(scene_context) = scene_context {
                scene_context.finish_entity::<TMessage, TResponse>(entity_id);
            }
        };
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
    /// Requests that we send messages to a channel for a particular entity
    ///
    pub (crate) fn send_to<TMessage, TResponse>(&mut self, entity_id: EntityId) -> Result<EntityChannel<TMessage, TResponse>, EntityChannelError> 
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send, 
    {
        // Try to retrieve the entity
        let entity = self.entities.get(&entity_id);
        let entity = if let Some(entity) = entity { entity } else { return Err(EntityChannelError::NoSuchEntity); };

        // TODO: attach to a default channel if the entity doesn't have this channel
        // TODO: default channels need to know how to upgrade to the 'real' channel if one is created
        // TODO: default channels should close for an entity if the entity is shut down
        
        // Attach to the channel in the entity that belongs to this stream type
        let channel = entity.lock().unwrap().attach_channel();
        let channel = if let Some(channel) = channel { channel } else { return Err(EntityChannelError::NotListening); };

        Ok(channel)
    }

    ///
    /// Called when an entity in this context has finished
    ///
    pub (crate) fn finish_entity(&mut self, entity_id: EntityId) {
        self.entities.remove(&entity_id);
    }
}

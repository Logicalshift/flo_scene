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

use std::any::{TypeId, Any};
use std::sync::*;
use std::collections::{HashMap};

///
/// The scene core represents the state shared between all entities in a scene
///
pub struct SceneCore {
    /// The entities that are available in this core
    pub (super) entities: HashMap<EntityId, EntityCore>,

    /// The default channel to use for a particular channel type
    pub (super) default_channel: HashMap<TypeId, Box<dyn Send + Any>>,

    /// The job queues for default types
    pub (super) default_queues: HashMap<TypeId, Arc<JobQueue>>,

    /// Futures waiting to run the entities in this scene
    pub (super) waiting_futures: Vec<BoxFuture<'static, ()>>,

    /// Used by the scene that owns this core to request wake-ups (only one scene can be waiting for a wake up at once)
    pub (super) wake_scene: Option<oneshot::Sender<()>>,
}

impl Default for SceneCore {
    fn default() -> SceneCore {
        SceneCore {
            entities:           HashMap::new(),
            default_channel:    HashMap::new(),
            default_queues:     HashMap::new(),
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
        // Create the entity representation for this item
        let entity_id           = scene_context.entity().unwrap();
        let (channel, receiver) = EntityChannel::new(5);
        let entity              = self.entities.entry(entity_id).or_insert_with(|| EntityCore::default());

        let queue = entity.create_queue(&channel)?;
        entity.register_channel(channel)?;

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
    /// Creates a default behaviour for a particular message type
    ///
    /// This message type will be accepted for all entities in the scene
    ///
    pub (crate) fn create_default<TMessage, TResponse, TFn, TFnFuture>(&mut self, scene_context: Arc<SceneContext>, runtime: TFn) -> Result<(), CreateDefaultError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
        TFn:        'static + Send + FnOnce(BoxStream<'static, Message<(EntityId, TMessage), TResponse>>) -> TFnFuture,
        TFnFuture:  'static + Send + Future<Output = ()>,
    {
        let default_type = TypeId::of::<EntityChannel<TMessage, TResponse>>();

        // Check that the default is not already running
        if self.default_channel.contains_key(&default_type) {
            return Err(CreateDefaultError::AlreadyExists);
        }

        // Create the communications channel and the queue for this item
        let (channel, receiver) = EntityChannel::new(5);
        let queue               = scheduler().create_job_queue();

        // Add this as the default type for this channel type
        self.default_queues.insert(default_type, queue.clone());
        self.default_channel.insert(default_type, Box::new(channel));

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

            // When done, deregister the default type
            if let Some(scene_context) = scene_context {
                scene_context.finish_default::<TMessage, TResponse>();
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

        // Attach to the channel in the entity that belongs to this stream type
        // TODO: attach to a default channel if the entity doesn't have this channel
        // TODO: default channels need to know how to upgrade to the 'real' channel if one is created
        // TODO: default channels should close for an entity if the entity is shut down
        let channel = entity.attach_channel();
        let channel = if let Some(channel) = channel { channel } else { return Err(EntityChannelError::NotListening); };

        Ok(channel)
    }

    ///
    /// Called when an entity in this context has finished
    ///
    pub (crate) fn finish_entity<TMessage, TResponse>(&mut self, entity_id: EntityId)
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        // Fetch the entity
        let entity = self.entities.get_mut(&entity_id);

        if let Some(entity) = entity {
            // De-register this channel
            if !entity.deregister::<TMessage, TResponse>() {
                // Remove the entity from the core if it has no remaining channels
                self.entities.remove(&entity_id);
            }
        }
    }

    ///
    /// Called when an default channel in this context has finished
    ///
    pub (crate) fn finish_default<TMessage, TResponse>(&mut self)
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        let default_type = TypeId::of::<EntityChannel<TMessage, TResponse>>();

        self.default_channel.remove(&default_type);
        self.default_queues.remove(&default_type);
    }
}

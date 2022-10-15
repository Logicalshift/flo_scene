use super::entity_core::*;
use super::entity_receiver::*;
use super::background_future::*;
use super::map_from_entity_type::*;

use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::ergonomics::*;
use crate::simple_entity_channel::*;
use crate::context::*;
use crate::standard_components::*;

use ::desync::scheduler::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::stream::{BoxStream};

use std::any::{TypeId};
use std::sync::*;
use std::sync::atomic::*;
use std::collections::{HashMap};

// TODO: way to map messages via a collection (or a stream?)
//      (could make it so that collection entities can take any collection, including a 1-item thing?)
//      (or make it so that channel always receive collections of requests)
// TODO: way to convert streams of JSON to entity messages

///
/// The scene core represents the state shared between all entities in a scene
///
pub struct SceneCore {
    /// The entities that are available in this core
    pub (super) entities: HashMap<EntityId, EntityCore>,

    /// The background futures, if they're available for the entity
    entity_background_futures: HashMap<EntityId, Weak<Mutex<BackgroundFutureCore>>>,

    /// Futures waiting to run the entities in this scene
    pub (super) waiting_futures: Vec<SchedulerFuture<()>>,

    /// Used by the scene that owns this core to request wake-ups (only one scene can be waiting for a wake up at once)
    pub (super) wake_scene: Option<oneshot::Sender<()>>,

    /// The number of entities that are currently running or which have a message waiting
    active_entity_count: Arc<AtomicIsize>,

    /// Provides a function for mapping from one entity channel type to another, based on the message type
    map_for_message: HashMap<TypeId, HashMap<TypeId, MapFromEntityType>>,

    /// The current state for the heartbeat of this scene
    heartbeat_state: HeartbeatState,

    /// Scheduler queue used for dispatching background messages
    pub (crate) message_queue: Arc<JobQueue>,
}

impl Default for SceneCore {
    fn default() -> SceneCore {
        SceneCore {
            entities:                   HashMap::new(),
            entity_background_futures:  HashMap::new(),
            waiting_futures:            vec![],
            wake_scene:                 None,
            active_entity_count:        Arc::new(AtomicIsize::new(0)),
            map_for_message:            HashMap::new(),
            heartbeat_state:            HeartbeatState::Tick,
            message_queue:              scheduler().create_job_queue(),
        }
    }
}

impl SceneCore {
    ///
    /// Sends a message using the background message processing queue
    ///
    pub (crate) fn send_background_message<TChannel>(&self, mut sender: TChannel, message: TChannel::Message) 
    where
        TChannel:           'static + Send + EntityChannel,
        TChannel::Message:  'static + Send,
    {
        scheduler().future_desync(&self.message_queue, move || async move {
            sender.send(message).await.ok()
        }).detach();
    }

    ///
    /// Creates an entity that processes a particular kind of message
    ///
    pub (crate) fn create_entity<TMessage, TFn, TFnFuture>(&mut self, scene_context: Arc<SceneContext>, runtime: TFn) -> Result<SimpleEntityChannel<TMessage>, CreateEntityError>
    where
        TMessage:   'static + Send,
        TFn:        'static + Send + FnOnce(Arc<SceneContext>, BoxStream<'static, TMessage>) -> TFnFuture,
        TFnFuture:  'static + Send + Future<Output = ()>,
    {
        // The entity ID is specified in the supplied scene context
        let entity_id           = scene_context.entity().unwrap();

        // The entity must not already exist
        if self.entities.contains_key(&entity_id) { return Err(CreateEntityError::AlreadyExists); }

        // Create a future that informs the scene context when the entity is ready
        let (ready, waiting)    = oneshot::channel();
        let ready_context       = Arc::clone(&scene_context);
        let signal_when_ready   = async move {
            // Wait for the channel to become ready (or to timeout)
            waiting.await.ok();

            // Tell the scene context that the entity is ready
            ready_context.ready_entity(entity_id);
        };

        // Create the channel and the entity
        let entity_future       = BackgroundFuture::new(Arc::clone(&scene_context));
        let (channel, receiver) = SimpleEntityChannel::new(entity_id, 5);
        let receiver            = EntityReceiver::new(receiver, &self.active_entity_count, Some(ready));
        let entity              = EntityCore::new(channel.clone());
        let queue               = entity.queue();

        self.entities.insert(entity_id, entity);
        self.entity_background_futures.insert(entity_id, Arc::downgrade(&entity_future.core()));

        // Start the future running
        let future              = async move {
            // Signal when the entity is ready
            scene_context.run_in_background(signal_when_ready).ok();

            // Tell the entity registry about the entity that was just created
            if entity_id != ENTITY_REGISTRY {
                // We usually don't let the entity start until it's definitely associated with the registry
                scene_context.send::<_>(ENTITY_REGISTRY, InternalRegistryRequest::CreatedEntity(entity_id, TypeId::of::<TMessage>())).await.ok();
            } else {
                // The entity registry itself might have a full queue by the time it gets around to registering itself: avoid blocking here by sending the request in the background
                let send = scene_context.send(ENTITY_REGISTRY, InternalRegistryRequest::CreatedEntity(entity_id, TypeId::of::<TMessage>()));
                scene_context.run_in_background(async move {
                    send.await.ok();
                }).ok();
            }

            // Create and run the actual runtime future
            let runtime_future = runtime(Arc::clone(&scene_context), receiver.boxed());
            runtime_future.await;

            // Notify the registry that the entity no longer exists
            scene_context.send(ENTITY_REGISTRY, InternalRegistryRequest::DestroyedEntity(entity_id)).await.ok();

            // Finish_entity calls back into the core to remove the entity from the list (note this calls stop() so this must be done last in the entity future)
            scene_context.finish_entity::<TMessage>(entity_id);
        };
        entity_future.core().add_future(future);

        // Queue a request in the runtime that we will run the entity
        let queued_future = scheduler().future_desync(&queue, move || entity_future);
        self.waiting_futures.push(queued_future);

        // Wake up the scene so it can schedule this future
        if let Some(wake_scene) = self.wake_scene.take() {
            wake_scene.send(()).ok();
        }

        Ok(channel)
    }

    ///
    /// Specifies that if an entity accepts messages in the format `TOriginalMessage` that these can be converted to `TNewMessage`
    ///
    pub (crate) fn convert_message<TOriginalMessage, TNewMessage>(&mut self)
    where
        TOriginalMessage:   'static + Send + Into<TNewMessage>,
        TNewMessage:        'static + Send,
    {
        // Create a converter from TOriginalMessage to TNewMessage
        let converter       = MapFromEntityType::new::<TOriginalMessage, TNewMessage>();
        let original_type   = TypeId::of::<TOriginalMessage>();
        let new_type        = TypeId::of::<TNewMessage>();

        // Any entity that accepts TNewMessage can also accept TOriginalMessage
        self.map_for_message.entry(new_type).or_insert_with(|| HashMap::new())
            .insert(original_type, converter);
    }

    ///
    /// Requests that we send messages to a channel for a particular entity
    ///
    pub (crate) fn send_to<TMessage>(&mut self, entity_id: EntityId) -> Result<BoxedEntityChannel<'static, TMessage>, EntityChannelError> 
    where
        TMessage:   'static + Send,
    {
        // Try to retrieve the entity
        let entity = self.entities.get(&entity_id);
        let entity = if let Some(entity) = entity { entity } else { return Err(EntityChannelError::NoSuchEntity); };
        
        // Attach to the channel in the entity that belongs to this stream type
        let channel = entity.attach_channel();
        
        if let Some(channel) = channel { 
            // Return the direct channel
            Ok(channel.boxed()) 
        } else {
            // Attempt to convert the message if possible
            let target_message      = entity.message_type_id();
            let source_message      = TypeId::of::<TMessage>();
            let message_converter   = self.map_for_message.get(&target_message).and_then(|target_hash| target_hash.get(&source_message));

            match message_converter {
                Some(message_converter) => {
                    // We have to go via an AnyEntityChannel as we don't have a place that knows all of the types
                    let any_channel         = entity.attach_channel_any();

                    // Convert the message
                    let message_conversion  = message_converter.conversion_function::<TMessage>().unwrap();
                    let channel             = any_channel.map(move |message| message_conversion(message));

                    Ok(channel.boxed())
                }

                None => {
                    Err(EntityChannelError::WrongChannelType(entity.message_type_name()))
                },
            }
        }
    }

    ///
    /// Adds a future to run in the background of this entity
    ///
    pub fn run_in_background(&self, entity_id: EntityId, future: impl 'static + Send + Future<Output=()>) -> Result<(), EntityFutureError> {
        if let Some(background_future) = self.entity_background_futures.get(&entity_id).and_then(|weak| weak.upgrade()) {
            background_future.add_future(future);
            Ok(())
        } else {
            Err(EntityFutureError::NoSuchEntity)
        }
    }

    ///
    /// Signals that an entity is ready
    ///
    pub (crate) fn ready_entity(&mut self, entity_id: EntityId) {
        if let Some(entity) = self.entities.get_mut(&entity_id) {
            // Signal that the entity is ready
            entity.signal_ready();
        }
    }

    ///
    /// 'Seals' an entity, making it so it will continue running but will not allow new channels to be opened to it
    ///
    /// (The entity registry won't report that the entity has been sealed)
    ///
    pub (crate) fn seal_entity(&mut self, entity_id: EntityId) -> Result<(), EntityChannelError> {
        if let Some(_entity) = self.entities.remove(&entity_id) {
            Ok(())
        } else {
            Err(EntityChannelError::NoSuchEntity)
        }
    }

    ///
    /// Stops an entity by forcibly stopping its futures
    ///
    /// This is a 'hard' stop that will drop the futures in their current state (if the future is running
    /// this will happen once it stops)
    ///
    pub (crate) fn stop_entity(&mut self, entity_id: EntityId) -> Result<(), EntityChannelError> {
        // Stop the background future if there is one
        if let Some(background_future) = self.entity_background_futures.remove(&entity_id).and_then(|weak| weak.upgrade()) {
            background_future.stop();
        }

        // Stop the entity, if there is one
        if let Some(entity) = self.entities.remove(&entity_id) {
            // TODO: this doesn't inform the entity registry that this entity has been shut down
            entity.stop();

            Ok(())
        } else {
            Err(EntityChannelError::NoSuchEntity)
        }
    }

    ///
    /// Stops an entity by closing its main channel
    ///
    /// This is a 'soft' stop operation, so the entity may keep accepting connections until it terminates
    /// its inner loop. The channel will receive a '
    ///
    pub (crate) fn close_entity(&mut self, entity_id: EntityId) -> Result<(), EntityChannelError> {
        if let Some(entity) = self.entities.get_mut(&entity_id) {
            entity.close();

            Ok(())
        } else {
            Err(EntityChannelError::NoSuchEntity)
        }
    }

    ///
    /// Called when an entity in this context has finished
    ///
    pub (crate) fn finish_entity(&mut self, entity_id: EntityId) {
        if let Some(background_future) = self.entity_background_futures.remove(&entity_id).and_then(|weak| weak.upgrade()) {
            background_future.stop();
        }

        if let Some(entity) = self.entities.remove(&entity_id) {
            entity.stop();
        }
    }

    ///
    /// All the entities in the scene are waiting for new messages
    ///
    pub (crate) async fn send_heartbeat(&mut self) {
        match &self.heartbeat_state {
            HeartbeatState::Tick => {
                // Request the heartbeat channel
                if let Ok(mut heartbeat_channel) = self.send_to::<InternalHeartbeatRequest>(HEARTBEAT) {
                    // The messages resulting from a heartbeat shouldn't generate a heartbeat themselves
                    self.heartbeat_state = HeartbeatState::Tock;

                    // Send a heartbeat request
                    if heartbeat_channel.send(InternalHeartbeatRequest::GenerateHeartbeat).await.is_err() {
                        // Failed to actually generate the heartbeat
                        self.heartbeat_state = HeartbeatState::Tick;
                    }
                }
            }

            HeartbeatState::Tock => {
                // The messages generated from a heartbeat have finished
                self.heartbeat_state = HeartbeatState::Tick;
            }
        }
    }
}

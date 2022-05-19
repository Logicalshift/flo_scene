use super::entity_core::*;
use super::entity_receiver::*;
use super::background_future::*;
use super::map_from_entity_type::*;
use super::map_into_entity_type::*;

use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::entity_channel_ext::*;
use crate::simple_entity_channel::*;
use crate::message::*;
use crate::context::*;
use crate::standard_components::*;

use ::desync::scheduler::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::stream::{BoxStream};
use futures::future;
use futures::future::{BoxFuture};

use std::any::{TypeId};
use std::sync::*;
use std::sync::atomic::*;
use std::collections::{HashMap};

// TODO: way to map messages via a collection (or a stream?) - for entities with a () response 
//      (could make it so that collection entities can take any collection, including a 1-item thing?)
//      (or make it so that channel always receive collections of requests)
// TODO: way to add futures that run in the background of an entity
// TODO: way to convert streams of JSON to entity messages

///
/// The scene core represents the state shared between all entities in a scene
///
pub struct SceneCore {
    /// The entities that are available in this core
    pub (super) entities: HashMap<EntityId, Arc<Mutex<EntityCore>>>,

    /// Futures waiting to run the entities in this scene
    pub (super) waiting_futures: Vec<BackgroundFuture>,

    /// Used by the scene that owns this core to request wake-ups (only one scene can be waiting for a wake up at once)
    pub (super) wake_scene: Option<oneshot::Sender<()>>,

    /// The number of entities that are currently running or which have a message waiting
    active_entity_count: Arc<AtomicIsize>,

    /// Provides a function for mapping from one entity channel type to another, based on the message type
    map_for_message: HashMap<TypeId, HashMap<TypeId, MapFromEntityType>>,

    /// Provides a function for mapping from one entity channel type to another, based on the response type
    map_for_response: HashMap<TypeId, HashMap<TypeId, MapIntoEntityType>>,

    /// The current state for the heartbeat of this scene
    heartbeat_state: HeartbeatState,
}

impl Default for SceneCore {
    fn default() -> SceneCore {
        SceneCore {
            entities:               HashMap::new(),
            waiting_futures:        vec![],
            wake_scene:             None,
            active_entity_count:    Arc::new(AtomicIsize::new(0)),
            map_for_message:        HashMap::new(),
            map_for_response:       HashMap::new(),
            heartbeat_state:        HeartbeatState::Tick,
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
        let entity_future       = BackgroundFuture::new();
        let (channel, receiver) = SimpleEntityChannel::new(5);
        let receiver            = EntityReceiver::new(receiver, &self.active_entity_count);
        let entity              = Arc::new(Mutex::new(EntityCore::new(channel)));
        let queue               = entity.lock().unwrap().queue();

        self.entities.insert(entity_id, entity);

        // Start the future running
        let future              = async move {
            // Tell the entity registry about the entity that was just created
            scene_context.send_without_waiting(ENTITY_REGISTRY, InternalRegistryRequest::CreatedEntity(entity_id, TypeId::of::<TMessage>(), TypeId::of::<TResponse>())).await.ok();

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
                // Finish_entity calls back into the core to remove the entity from the list
                scene_context.finish_entity::<TMessage, TResponse>(entity_id);

                // Notify the registry that the entity no longer exists
                scene_context.send_without_waiting(ENTITY_REGISTRY, InternalRegistryRequest::DestroyedEntity(entity_id)).await.ok();
            }
        };
        entity_future.core().add_future(future);

        // Queue a request in the runtime that we will run the entity
        self.waiting_futures.push(entity_future);

        // Wake up the scene so it can schedule this future
        if let Some(wake_scene) = self.wake_scene.take() {
            wake_scene.send(()).ok();
        }

        Ok(())
    }

    ///
    /// Specifies that if an entity accepts messages in the format `TOriginalMessage` that these can be converted to `TNewMessage`
    ///
    pub (crate) fn convert_message<TOriginalMessage, TNewMessage>(&mut self)
    where
        TOriginalMessage:   'static + Send,
        TNewMessage:        'static + Send + From<TOriginalMessage>,
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
    /// Specifies that if an entity accepts responses in the format `TOriginalResponse` that these can be converted to `TNewResponse`
    ///
    pub (crate) fn convert_response<TOriginalResponse, TNewResponse>(&mut self)
    where
        TOriginalResponse:  'static + Send + Into<TNewResponse>,
        TNewResponse:       'static + Send,
    {
        // Create a converter from TOriginalResponse to TNewResponse
        let converter       = MapIntoEntityType::new::<TOriginalResponse, TNewResponse>();
        let original_type   = TypeId::of::<TOriginalResponse>();
        let new_type        = TypeId::of::<TNewResponse>();

        // Any entity that accepts TNewResponse can also accept TOriginalResponse
        self.map_for_response.entry(original_type).or_insert_with(|| HashMap::new())
            .insert(new_type, converter);
    }

    ///
    /// Requests that we send messages to a channel for a particular entity
    ///
    pub (crate) fn send_to<TMessage, TResponse>(&mut self, entity_id: EntityId) -> Result<BoxedEntityChannel<'static, TMessage, TResponse>, EntityChannelError> 
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send, 
    {
        // Try to retrieve the entity
        let entity = self.entities.get(&entity_id);
        let entity = if let Some(entity) = entity { entity } else { return Err(EntityChannelError::NoSuchEntity); };
        
        // Attach to the channel in the entity that belongs to this stream type
        let channel = entity.lock().unwrap().attach_channel();
        
        if let Some(channel) = channel { 
            // Return the direct channel
            Ok(channel.boxed()) 
        } else {
            // Attempt to convert the message if possible
            let target_message      = entity.lock().unwrap().message_type_id();
            let source_message      = TypeId::of::<TMessage>();
            let message_converter   = self.map_for_message.get(&target_message).and_then(|target_hash| target_hash.get(&source_message));

            // ... also possibly convert the responce
            let source_response     = entity.lock().unwrap().response_type_id();
            let target_response     = TypeId::of::<TResponse>();
            let response_converter  = self.map_for_response.get(&source_response).and_then(|target_hash| target_hash.get(&target_response));

            match (message_converter, response_converter) {
                (Some(message_converter), None) => {
                    // Response types must match
                    if source_response != target_response {
                        return Err(EntityChannelError::WrongResponseType(entity.lock().unwrap().response_type_name()));
                    }

                    // We have to go via an AnyEntityChannel as we don't have a place that knows all of the types
                    let any_channel         = entity.lock().unwrap().attach_channel_any();

                    // Convert from TMessage to a boxed 'Any' function
                    let conversion_function = message_converter.conversion_function::<TMessage>().unwrap();

                    // Map from the source message to the 'Any' message and from the 'Any' response back to the 'real' response
                    let channel             = any_channel.map(
                        move |message| conversion_function(message), 
                        move |mut response| response.downcast_mut::<Option<TResponse>>().unwrap().take().unwrap());

                    Ok(channel.boxed())
                }

                (None, Some(response_converter)) => {
                    // Message types must match
                    if source_message != target_message {
                        return Err(EntityChannelError::WrongMessageType(entity.lock().unwrap().message_type_name()));
                    }

                    // We have to go via an AnyEntityChannel as we don't have a place that knows all of the types
                    let any_channel         = entity.lock().unwrap().attach_channel_any();

                    // Convert from 'Any' to TResponse
                    let conversion_function = response_converter.conversion_function::<TResponse>().unwrap();

                    // Map from the source response to the 'Any' response and from the 'Any' response back to the 'real' response
                    let channel             = any_channel.map(
                        move |message: TMessage| Box::new(Some(message)), 
                        move |response| conversion_function(response).unwrap());

                    Ok(channel.boxed())
                }

                (Some(message_converter), Some(response_converter)) => {
                    // We have to go via an AnyEntityChannel as we don't have a place that knows all of the types
                    let any_channel         = entity.lock().unwrap().attach_channel_any();

                    // Convert the message and the response
                    let message_conversion  = message_converter.conversion_function::<TMessage>().unwrap();
                    let response_conversion = response_converter.conversion_function::<TResponse>().unwrap();

                    // Map from the source response to the 'Any' response and from the 'Any' response back to the 'real' response
                    let channel             = any_channel.map(
                        move |message| message_conversion(message), 
                        move |response| response_conversion(response).unwrap());

                    Ok(channel.boxed())
                }

                (None, None) => {
                    let entity = entity.lock().unwrap();

                    Err(EntityChannelError::WrongChannelType(entity.message_type_name(), entity.response_type_name()))
                },
            }
        }
    }

    ///
    /// Called when an entity in this context has finished
    ///
    pub (crate) fn finish_entity(&mut self, entity_id: EntityId) {
        self.entities.remove(&entity_id);
    }

    ///
    /// All the entities in the scene are waiting for new messages
    ///
    pub (crate) async fn send_heartbeat(&mut self) {
        match &self.heartbeat_state {
            HeartbeatState::Tick => {
                // Request the heartbeat channel
                if let Ok(mut heartbeat_channel) = self.send_to::<InternalHeartbeatRequest, ()>(HEARTBEAT) {
                    // The messages resulting from a heartbeat shouldn't generate a heartbeat themselves
                    self.heartbeat_state = HeartbeatState::Tock;

                    // Send a heartbeat request
                    if heartbeat_channel.send_without_waiting(InternalHeartbeatRequest::GenerateHeartbeat).await.is_err() {
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

use super::entity_ids::*;

use crate::error::*;
use crate::context::*;
use crate::message::*;
use crate::entity_id::*;
use crate::entity_channel::*;

use futures::prelude::*;

use std::any::{TypeId};
use std::sync::*;
use std::collections::{HashMap, HashSet};

///
/// Describes the message and response for an entity channel
///
#[derive(Clone, Debug, PartialEq)]
pub struct EntityChannelType {
    pub message_type:   TypeId,
    pub response_type:  TypeId,
}

///
/// Requests that can be made for the entity registry 
///
#[derive(Debug)]
pub enum EntityRegistryRequest {
    ///
    /// Sends updates for all entities to the specified entity channel
    ///
    TrackEntities(BoxedEntityChannel<'static, EntityUpdate, ()>),

    ///
    /// As for TrackEntities but only for those that use a particular channel type
    ///
    TrackEntitiesWithType(BoxedEntityChannel<'static, EntityUpdate, ()>, EntityChannelType),
}

///
/// The entity update message that the entity registry will send to any entities that have asked to track the registry
///
#[derive(Clone, Debug, PartialEq)]
pub enum EntityUpdate {
    /// A new entity was created
    CreatedEntity(EntityId),

    /// An entity was destroyed
    DestroyedEntity(EntityId),
}

///
/// Requests that can be made for the entity registry
///
#[derive(Debug)]
pub (crate) enum InternalRegistryRequest {
    ///
    /// Opens an entity update channel (of type `EntityChannel<EntityUpdate, ()>`) to the specified entity and sends updates to indicate when entities are added or
    /// removed to/from the scene
    ///
    TrackEntities(BoxedEntityChannel<'static, EntityUpdate, ()>),

    ///
    /// Opens an entity update channel (of type `EntityChannel<EntityUpdate, ()>`) to the specified entity and sends updates to indicate when any entity that can 
    /// accept a channel of this type is created or destroyed
    ///
    TrackEntitiesWithType(BoxedEntityChannel<'static, EntityUpdate, ()>, EntityChannelType),

    ///
    /// Sent from the scene core: a new entity was created (with the specified message and response types for its main stream)
    ///
    CreatedEntity(EntityId, TypeId, TypeId),

    ///
    /// Send from the scene 
    ///
    DestroyedEntity(EntityId),

    ///
    /// Indicates that one message type can be converted to another
    ///
    ConvertMessage(TypeId, TypeId),

    ///
    /// Indicates that one response type can be converted to another
    ///
    ConvertResponse(TypeId, TypeId),
}

impl From<EntityRegistryRequest> for InternalRegistryRequest {
    fn from(req: EntityRegistryRequest) -> InternalRegistryRequest {
        match req {
            EntityRegistryRequest::TrackEntities(entity_id)                         => InternalRegistryRequest::TrackEntities(entity_id),
            EntityRegistryRequest::TrackEntitiesWithType(entity_id, channel_type)   => InternalRegistryRequest::TrackEntitiesWithType(entity_id, channel_type),
        }
    }
}

///
/// State for the entity registry
///
struct RegistryState {
    /// The entities and their message types
    entities: HashMap<EntityId, EntityChannelType>,

    /// Which messages can be converted to which other types
    convert_message: HashMap<TypeId, HashSet<TypeId>>,

    /// Which responses can be converted to which other types
    convert_response: HashMap<TypeId, HashSet<TypeId>>,
}

impl EntityChannelType {
    ///
    /// Creates a new entity channel type from a pair of type IDs representing the message and the response types
    ///
    pub fn new(message_type: TypeId, response_type: TypeId) -> EntityChannelType {
        EntityChannelType {
            message_type,
            response_type
        }
    }

    ///
    /// Creates a new entity channel type from a pair of types
    ///
    pub fn of<MessageType, ResponseType>() -> EntityChannelType
    where
        MessageType:    'static,
        ResponseType:   'static
    {
        Self::new(TypeId::of::<MessageType>(), TypeId::of::<ResponseType>())
    }
}

impl RegistryState {
    ///
    /// Returns true if an entity that has a channel type will convert from the given match type
    ///
    fn can_convert_type(&self, channel_type: &EntityChannelType, match_type: &EntityChannelType) -> bool {
        let message_match = channel_type.message_type == match_type.message_type 
            || self.convert_message.get(&match_type.message_type).map(|types| types.contains(&channel_type.message_type)).unwrap_or(false);
        let response_match = channel_type.response_type == match_type.response_type
            || self.convert_response.get(&channel_type.response_type).map(|types| types.contains(&match_type.response_type)).unwrap_or(false);

        message_match && response_match
    }
}

///
/// Creates the entity registry in a context
///
pub fn create_entity_registry_entity(context: &Arc<SceneContext>) -> Result<(), CreateEntityError> {
    // Programs outside of flo_scene can make requests from the `EntityRegistryRequest` API
    context.convert_message::<EntityRegistryRequest, InternalRegistryRequest>()?;

    let mut state = RegistryState {
        entities:           HashMap::new(),
        convert_message:    HashMap::new(),
        convert_response:   HashMap::new(),
    };

    let mut trackers: Vec<Option<BoxedEntityChannel<'static, EntityUpdate, ()>>>                            = vec![];
    let mut typed_trackers: Vec<Option<(EntityChannelType, BoxedEntityChannel<'static, EntityUpdate, ()>)>> = vec![];

    // Create the entity registry (it's just a stream entity)
    context.create_entity(ENTITY_REGISTRY, move |context, mut requests| async move {
        // Read requests from the stream
        while let Some(request) = requests.next().await {
            use InternalRegistryRequest::*;

            let request: Message<InternalRegistryRequest, ()>   = request;
            let (request, response)                             = request.take();

            match request {
                CreatedEntity(entity_id, message_type, response_type) => {
                    let entity_id       = entity_id;
                    let message_type    = message_type;
                    let response_type   = response_type;

                    // Add to the list of entities
                    state.entities.insert(entity_id, EntityChannelType::new(message_type, response_type));

                    // Inform the trackers
                    // TODO: tidy up any trackers that are no longer responding
                    let mut futures             = vec![];

                    for maybe_tracker in trackers.iter_mut() {
                        if let Some(tracker) = maybe_tracker {
                            // Send that a new entity has been created to the tracker
                            futures.push(tracker.send_without_waiting(EntityUpdate::CreatedEntity(entity_id)));
                        }
                    }

                    let entity_type = EntityChannelType::new(message_type, response_type);
                    for maybe_tracker in typed_trackers.iter_mut() {
                        if let Some((match_type, tracker)) = maybe_tracker {
                            if state.can_convert_type(&entity_type, match_type) {
                                // Send that a new entity has been created to the tracker
                                futures.push(tracker.send_without_waiting(EntityUpdate::CreatedEntity(entity_id)));
                            }
                        }
                    }

                    // If any of the trackers have not completed sending their messages, put the task into the background
                    if !futures.is_empty() {
                        context.run_in_background(async move { future::join_all(futures).await; });
                    }

                    // Once all of the trackers have been informed of the new entity, respond OK
                    response.send(()).ok();
                }

                DestroyedEntity(entity_id) => {
                    let entity_id       = entity_id;

                    // We respond OK before the entity finishes being destroyed
                    response.send(()).ok();

                    // Remove the entity from the list we're tracking
                    // TODO: tidy up any trackers that are no longer responding
                    if let Some(entity_type) = state.entities.remove(&entity_id) {
                        // Inform the trackers that this entity has been removed
                        let mut futures = vec![];

                        for maybe_tracker in trackers.iter_mut() {
                            if let Some(tracker) = maybe_tracker {
                                // Send that a new entity has been destroyed to the tracker
                                futures.push(tracker.send_without_waiting(EntityUpdate::DestroyedEntity(entity_id)));
                            }
                        }

                        for maybe_tracker in typed_trackers.iter_mut() {
                            if let Some((match_type, tracker)) = maybe_tracker {
                                if state.can_convert_type(&entity_type, match_type) {
                                    // Send that a new entity has been destroyed to the tracker
                                    futures.push(tracker.send_without_waiting(EntityUpdate::DestroyedEntity(entity_id)));
                                }
                            }
                        }

                        // If any of the trackers have not completed sending their messages, put the task into the background
                        if !futures.is_empty() {
                            context.run_in_background(async move { future::join_all(futures).await; });
                        }
                    }
                }

                ConvertMessage(source_type, target_type) => {
                    // Store that something that accepts 'source_type' can also accept 'target_type'
                    state.convert_message.entry(target_type).or_insert_with(|| HashSet::new()).insert(source_type);
                    response.send(()).ok();
                }

                ConvertResponse(source_type, target_type) => {
                    // Store that something that can respond with 'source_type' can also respond with 'target_type'
                    state.convert_response.entry(source_type).or_insert_with(|| HashSet::new()).insert(target_type);
                    response.send(()).ok();
                }

                TrackEntities(channel)   => {
                    // Send the list of entities to the channel
                    let mut channel = channel;
                    let mut futures = vec![];
                    for existing_entity_id in state.entities.keys().cloned() {
                        futures.push(channel.send_without_waiting(EntityUpdate::CreatedEntity(existing_entity_id)));
                    }

                    if !futures.is_empty() {
                        context.run_in_background(async move { future::join_all(futures).await; });
                    }

                    // All the entities are being tracked
                    response.send(()).ok();

                    // Add to the list of trackers
                    trackers.push(Some(channel));
                }

                TrackEntitiesWithType(channel, channel_type) => {
                    // Send the list of entities that match this type to the channel
                    let mut channel = channel;
                    let mut futures = vec![];
                    for (existing_entity_id, existing_type) in state.entities.iter() {
                        if state.can_convert_type(existing_type, &channel_type) {
                            futures.push(channel.send_without_waiting(EntityUpdate::CreatedEntity(*existing_entity_id)));
                        }
                    }

                    if !futures.is_empty() {
                        context.run_in_background(async move { future::join_all(futures).await; });
                    }

                    response.send(()).ok();

                    // Add to the list of typed trackers
                    typed_trackers.push(Some((channel_type, channel)));
                }
            }
        }
    })?;

    Ok(())
}

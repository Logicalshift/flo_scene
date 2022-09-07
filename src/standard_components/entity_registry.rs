use super::entity_ids::*;

use crate::error::*;
use crate::context::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::ergonomics::*;

use futures::prelude::*;

use std::any::{TypeId};
use std::sync::*;
use std::collections::{HashMap, HashSet};

///
/// Describes the message for an entity channel
///
#[derive(Clone, Debug, PartialEq)]
pub struct EntityChannelType {
    pub message_type:   TypeId,
}

///
/// Requests that can be made for the entity registry 
///
#[derive(Debug)]
pub enum EntityRegistryRequest {
    ///
    /// Sends updates for all entities to the specified entity channel
    ///
    TrackEntities(BoxedEntityChannel<'static, EntityUpdate>),

    ///
    /// As for TrackEntities but only for those that use a particular channel type
    ///
    TrackEntitiesWithType(BoxedEntityChannel<'static, EntityUpdate>, EntityChannelType),

    ///
    /// Retrieves the list of entities that exist at the time the message is received and then closes the channel
    ///
    /// This differs from TrackEntities in that this doesn't keep sending updates for entities that are created in the
    /// future.
    ///
    GetEntities(BoxedEntityChannel<'static, EntityId>),
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
    TrackEntities(BoxedEntityChannel<'static, EntityUpdate>),

    ///
    /// Opens an entity update channel (of type `EntityChannel<EntityUpdate, ()>`) to the specified entity and sends updates to indicate when any entity that can 
    /// accept a channel of this type is created or destroyed
    ///
    TrackEntitiesWithType(BoxedEntityChannel<'static, EntityUpdate>, EntityChannelType),

    ///
    /// Retrieves the list of entities that exist at the time the message is received and then closes the channel
    ///
    /// This differs from TrackEntities in that this doesn't keep sending updates for entities that are created in the
    /// future.
    ///
    GetEntities(BoxedEntityChannel<'static, EntityId>),

    ///
    /// Sent from the scene core: a new entity was created (with the specified message type for its main stream)
    ///
    CreatedEntity(EntityId, TypeId),

    ///
    /// Send from the scene 
    ///
    DestroyedEntity(EntityId),

    ///
    /// Indicates that one message type can be converted to another
    ///
    ConvertMessage(TypeId, TypeId),
}

impl From<EntityRegistryRequest> for InternalRegistryRequest {
    fn from(req: EntityRegistryRequest) -> InternalRegistryRequest {
        match req {
            EntityRegistryRequest::GetEntities(channel)                         => InternalRegistryRequest::GetEntities(channel),
            EntityRegistryRequest::TrackEntities(channel)                       => InternalRegistryRequest::TrackEntities(channel),
            EntityRegistryRequest::TrackEntitiesWithType(channel, channel_type) => InternalRegistryRequest::TrackEntitiesWithType(channel, channel_type),
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
}

impl EntityChannelType {
    ///
    /// Creates a new entity channel type from a pair of type IDs representing the message type
    ///
    pub fn new(message_type: TypeId) -> EntityChannelType {
        EntityChannelType {
            message_type,
        }
    }

    ///
    /// Creates a new entity channel type from a message type
    ///
    pub fn of<MessageType>() -> EntityChannelType
    where
        MessageType:    'static,
    {
        Self::new(TypeId::of::<MessageType>())
    }
}

impl RegistryState {
    ///
    /// Returns true if an entity that has a channel type will convert from the given match type
    ///
    fn can_convert_type(&self, channel_type: &EntityChannelType, match_type: &EntityChannelType) -> bool {
        let message_match = channel_type.message_type == match_type.message_type 
            || self.convert_message.get(&match_type.message_type).map(|types| types.contains(&channel_type.message_type)).unwrap_or(false);

        message_match 
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
    };

    let mut trackers: Vec<Option<BoxedEntityChannel<'static, EntityUpdate>>>                            = vec![];
    let mut typed_trackers: Vec<Option<(EntityChannelType, BoxedEntityChannel<'static, EntityUpdate>)>> = vec![];

    // Create the entity registry (it's just a stream entity)
    context.create_entity(ENTITY_REGISTRY, move |_context, mut requests| async move {
        // Read requests from the stream
        while let Some(request) = requests.next().await {
            use InternalRegistryRequest::*;

            let request: InternalRegistryRequest = request;

            // Remove any trackers that have been closed since the last message
            let mut removed_trackers = false;
            for maybe_tracker in trackers.iter_mut() {
                if let Some(tracker) = maybe_tracker {
                    if tracker.is_closed() {
                        *maybe_tracker      = None;
                        removed_trackers    = true;
                    }
                }
            }

            for maybe_tracker in typed_trackers.iter_mut() {
                if let Some((_, tracker)) = maybe_tracker {
                    if tracker.is_closed() {
                        *maybe_tracker      = None;
                        removed_trackers    = true;
                    }
                }
            }

            if removed_trackers {
                trackers.retain(|tracker| tracker.is_some());
                typed_trackers.retain(|tracker| tracker.is_some());
            }

            // Process the actual request
            match request {
                CreatedEntity(entity_id, message_type) => {
                    let entity_id       = entity_id;
                    let message_type    = message_type;

                    // Add to the list of entities
                    state.entities.insert(entity_id, EntityChannelType::new(message_type));

                    // Inform the trackers
                    let mut futures             = vec![];

                    for maybe_tracker in trackers.iter_mut() {
                        if let Some(tracker) = maybe_tracker {
                            // Send that a new entity has been created to the tracker
                            futures.push(tracker.send_without_waiting(EntityUpdate::CreatedEntity(entity_id)));
                        }
                    }

                    let entity_type = EntityChannelType::new(message_type);
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
                        future::join_all(futures)
                            .map(|_| ())
                            .run_in_background()
                            .ok();
                    }
                }

                DestroyedEntity(entity_id) => {
                    let entity_id       = entity_id;

                    // Remove the entity from the list we're tracking
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
                            future::join_all(futures)
                                .map(|_| ())
                                .run_in_background()
                                .ok();
                            }
                    }
                }

                ConvertMessage(source_type, target_type) => {
                    // Store that something that accepts 'source_type' can also accept 'target_type'
                    state.convert_message.entry(target_type).or_insert_with(|| HashSet::new()).insert(source_type);
                }

                TrackEntities(channel) => {
                    // Send the list of entities to the channel
                    let mut channel = channel;
                    let mut futures = vec![];
                    for existing_entity_id in state.entities.keys().cloned() {
                        futures.push(channel.send_without_waiting(EntityUpdate::CreatedEntity(existing_entity_id)));
                    }

                    if !futures.is_empty() {
                        future::join_all(futures)
                            .map(|_| ())
                            .run_in_background()
                            .ok();
                    }

                    // All the entities are being tracked: add to the list of trackers
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
                        future::join_all(futures)
                            .map(|_| ())
                            .run_in_background()
                            .ok();
                    }

                    // Add to the list of typed trackers
                    typed_trackers.push(Some((channel_type, channel)));
                }

                GetEntities(channel) => {
                    // Send the list of entities to the channel
                    let mut channel = channel;
                    let mut futures = vec![];
                    for existing_entity_id in state.entities.keys().cloned() {
                        futures.push(channel.send_without_waiting(existing_entity_id));
                    }

                    if !futures.is_empty() {
                        future::join_all(futures)
                            .map(|_| ())
                            .run_in_background()
                            .ok();
                    }

                    // Dropping the channel will close it
                }
            }
        }
    })?;

    Ok(())
}

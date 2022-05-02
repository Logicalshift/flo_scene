use super::entity_ids::*;

use crate::error::*;
use crate::context::*;
use crate::entity_id::*;
use crate::entity_channel::*;

use futures::prelude::*;

use std::any::{TypeId};
use std::sync::*;
use std::collections::{HashMap};

///
/// Requests that can be made for the entity registry 
///
#[derive(Clone, Debug, PartialEq)]
pub enum EntityRegistryRequest {
    ///
    /// Opens an entity update channel (of type `EntityChannel<EntityUpdate, ()>`) to the specified entity and sends updates to indicate when entities are added or
    /// removed to/from the scene
    ///
    TrackEntities(EntityId),
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
#[derive(PartialEq)]
enum InternalRegistryRequest {
    ///
    /// Opens an entity update channel (of type `EntityChannel<EntityUpdate, ()>`) to the specified entity and sends updates to indicate when entities are added or
    /// removed to/from the scene
    ///
    TrackEntities(EntityId),

    ///
    /// Sent from the scene core: a new entity was created (with the specified message and response types for its main stream)
    ///
    CreatedEntity(EntityId, TypeId, TypeId),

    ///
    /// Indicates that one message type can be converted to another
    ///
    ConvertMessage(TypeId, TypeId),

    ///
    /// Indicates that one response type can be converted to another
    ///
    ConvertResponse(TypeId, TypeId),

    ///
    /// Send from the scene 
    ///
    DestroyedEntity(EntityId),
}

impl From<EntityRegistryRequest> for InternalRegistryRequest {
    fn from(req: EntityRegistryRequest) -> InternalRegistryRequest {
        match req {
            EntityRegistryRequest::TrackEntities(entity_id) => InternalRegistryRequest::TrackEntities(entity_id),
        }
    }
}

///
/// Creates the entity registry in a context
///
pub (crate) fn create_entity_registry(context: &Arc<SceneContext>) -> Result<(), CreateEntityError> {
    // Programs outside of flo_scene can make requests from the `EntityRegistryRequest` API
    context.convert_message::<EntityRegistryRequest, InternalRegistryRequest>()?;

    // State for the entity registry
    struct RegistryState {
        /// The entities and their message types
        entities: HashMap<EntityId, (TypeId, TypeId)>,

        /// Which messages can be converted to which other types
        convert_message: HashMap<TypeId, TypeId>,

        /// Which responses can be converted to which other types
        convert_response: HashMap<TypeId, TypeId>,

        /// The list of trackers for this registry (set to None for any trackers that have finished)
        trackers: Vec<Option<BoxedEntityChannel<'static, EntityUpdate, ()>>>,
    }

    let mut state = RegistryState {
        entities:           HashMap::new(),
        convert_message:    HashMap::new(),
        convert_response:   HashMap::new(),
        trackers:           vec![],
    };

    // Create the entity registry (it's just a stream entity)
    context.create_stream_entity(ENTITY_REGISTRY, move |mut requests| async move {
        // Read requests from the stream
        while let Some(request) = requests.next().await {
            use InternalRegistryRequest::*;
            let request: InternalRegistryRequest = request;

            match request {
                CreatedEntity(entity_id, message_type, response_type) => {
                    // Add to the list of entities
                    state.entities.insert(entity_id, (message_type, response_type));

                    // Inform the trackers (and tidy up any trackers that are no longer responding)
                    let mut trackers_finished = false;

                    for maybe_tracker in state.trackers.iter_mut() {
                        if let Some(tracker) = maybe_tracker {
                            // Send that a new entity has been created to the tracker
                            let send_result = tracker.send(EntityUpdate::CreatedEntity(entity_id)).await;

                            // Set to None if the result is an error
                            if send_result.is_err() {
                                trackers_finished   = true;
                                *maybe_tracker      = None;
                            }
                        }
                    }

                    // Clean out any trackers that are no longer tracking anything
                    if trackers_finished {
                        state.trackers.retain(|tracker| !tracker.is_none());
                    }
                }

                ConvertMessage(source_type, target_type) => {
                    // Store that something that accepts 'source_type' can also accept 'target_type'
                    state.convert_message.insert(source_type, target_type);
                }

                ConvertResponse(source_type, target_type) => {
                    // Store that something that can respond with 'source_type' can also respond with 'target_type'
                    state.convert_response.insert(source_type, target_type);
                }

                DestroyedEntity(entity_id) => {
                    // Remove the entity from the list we're tracking
                    if state.entities.remove(&entity_id).is_some() {
                        // Inform the trackers that this entity has been removed
                        let mut trackers_finished = false;

                        for maybe_tracker in state.trackers.iter_mut() {
                            if let Some(tracker) = maybe_tracker {
                                // Send that a new entity has been destroyed to the tracker
                                let send_result = tracker.send(EntityUpdate::DestroyedEntity(entity_id)).await;

                                // Set to None if the result is an error
                                if send_result.is_err() {
                                    trackers_finished   = true;
                                    *maybe_tracker      = None;
                                }
                            }
                        }

                        // Clean out any trackers that are no longer tracking anything
                        if trackers_finished {
                            state.trackers.retain(|tracker| !tracker.is_none());
                        }
                    }
                }

                TrackEntities(target)   => {
                    // Create a channel to the target
                    if let Ok(channel) = scene_send_to::<EntityUpdate, ()>(target) {
                        // Send the list of entities to the channel
                        let mut channel = channel;
                        for existing_entity_id in state.entities.keys().cloned() {
                            channel.send(EntityUpdate::CreatedEntity(existing_entity_id)).await.ok();
                        }

                        // Add to the list of trackers
                        state.trackers.push(Some(channel));
                    }
                }
            }
        }
    })?;

    Ok(())
}

use crate::entity_id::*;

use super::entity_registry::*;

///
/// The reason a scene is currently 'awake'
///
#[derive(Clone, Debug, PartialEq)]
pub (crate) enum HeartbeatState {
    /// Message queue awoken 'organically'
    Tick,

    /// Message queue awoken due to a heartbeat
    Tock,
}

///
/// The 'native' format for the heartbeat entity
///
#[derive(Clone, Debug, PartialEq)]
pub (crate) enum InternalHeartbeatRequest {
    /// From the scene core: indicates that a heartbeat has occurred
    GenerateHeartbeat,

    /// Request from the entity registry
    EntityUpdate(EntityUpdate),

    /// Send Heartbeat messages to the specified entity ID
    RequestHeartbeat(EntityId),
}

///
/// Requests that can be made of the heartbeat entity
///
#[derive(Clone, Debug, PartialEq)]
pub enum HeartbeatRequest {
    /// Send Heartbeat messages to the specified entity ID
    RequestHeartbeat(EntityId),
}

///
/// The heartbeat message
///
/// Typically an entity that needs to receive heartbeats would convert this into an internal message type
///
#[derive(Clone, Debug, PartialEq)]
pub struct Heartbeat;

impl From<HeartbeatRequest> for InternalHeartbeatRequest {
    fn from(req: HeartbeatRequest) -> InternalHeartbeatRequest {
        match req {
            HeartbeatRequest::RequestHeartbeat(entity_id)   => InternalHeartbeatRequest::RequestHeartbeat(entity_id),
        }
    }
}

impl From<EntityUpdate> for InternalHeartbeatRequest {
    fn from(req: EntityUpdate) -> InternalHeartbeatRequest {
        InternalHeartbeatRequest::EntityUpdate(req)
    }
}

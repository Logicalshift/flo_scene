use crate::entity_id::*;

use uuid::*;

///
/// UUID of the entity registry entity.
///
/// The entity registry is used to track entities as they're created and destroyed
///
pub const ENTITY_REGISTRY: EntityId = EntityId::well_known(uuid!["05FE1AC4-6B61-43CA-947C-2E67E465E2C5"]);

///
/// UUID of the heartbeat entity.
///
/// This can be used to request heartbeats. Heartbeats are generated whenever all of the channels for all of the 
/// entities in a scene have no more pending messages (other than )
///
pub const HEARTBEAT: EntityId       = EntityId::well_known(uuid!["C84E950C-0FA1-47C7-A453-C6C65B1BEEA9"]);

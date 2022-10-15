use crate::entity_id::*;

use uuid::*;

///
/// Entity created to run unit tests
///
pub const TEST_ENTITY: EntityId     = EntityId::well_known(uuid!["5B93BD5F-39F5-4B57-ABE9-DF593F331E86"]);

///
/// Entity used for illustrative examples
///
pub const EXAMPLE: EntityId  = EntityId::well_known(uuid!["078D0DCA-972A-40AC-AC79-F50EA16C1837"]);

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

///
/// UUID of the entity that controls the currently running scene
///
pub const SCENE_CONTROL: EntityId   = EntityId::well_known(uuid!["1A0EDDC4-5F99-4BC3-B646-DFD4B71F8B0E"]);

///
/// UUID of an entity that can provide timed events on request
///
pub const TIMER: EntityId           = EntityId::well_known(uuid!["F9F311F6-EAC0-4D7F-ACD1-39BEF2418376"]);

///
/// UUID of an entity that can provide log messages about a particular scene
///
pub const LOGGING: EntityId         = EntityId::well_known(uuid!["E197C07D-BC63-41B1-9B88-ACA4CCAF8B0E"]);

///
/// UUID of an entity that can manage the properties of other entities
///
pub const PROPERTIES: EntityId      = EntityId::well_known(uuid!["1702A40B-198B-4424-808A-68BF1BFA6451"]);

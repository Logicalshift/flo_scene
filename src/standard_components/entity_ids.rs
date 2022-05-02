use crate::entity_id::*;

use uuid::*;

///
/// UUID of the entity registry entity.
///
/// The entity registry is used to track entities as they're created and destroyed
///
pub const ENTITY_REGISTRY: EntityId = EntityId::well_known(uuid!["05FE1AC4-6B61-43CA-947C-2E67E465E2C5"]);

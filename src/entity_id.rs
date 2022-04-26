use uuid::*;

///
/// Uniquely identifies an entity
///
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct EntityId(Uuid);

impl EntityId {
    ///
    /// Creates a new, unique, entity ID
    ///
    pub fn new() -> EntityId {
        EntityId(Uuid::new_v4())
    }

    ///
    /// Creates an entity ID with a well-known UUID
    ///
    pub const fn well_known(uuid: Uuid) -> EntityId {
        EntityId(uuid)
    }
}

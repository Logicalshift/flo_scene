// TODO: entity to stop the scene
// TODO: entity to shut down other entities
// TODO: entity to generate timed events
// TODO: scripting entity
// TODO: HTTP server entity
// TODO: JSON streaming entity
// TODO: logging entity
// TODO: error reporting entity
// TODO: progress reporting entity
// TODO: named pipe entity (+ entity to introduce the contents of a named pipe as entities)
// TODO: entity to stop the scene (and other entities?)

// TODO: entity to contain properties/bindings (not really standard as we expect the user to create this)

mod entity_ids;
mod entity_registry;
mod heartbeat;

pub use self::entity_ids::*;
pub use self::entity_registry::*;
pub use self::heartbeat::*;

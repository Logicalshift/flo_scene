// * TODO: entity to stop the scene
// * TODO: entity to generate timed events
// * TODO: logging entity
// * TODO: entity to contain properties/bindings (not really standard as we expect the user to create this)

// TODO: entity to shut down other entities
// TODO: scripting entity
// TODO: HTTP server entity
// TODO: JSON streaming entity
// TODO: error reporting entity
// TODO: progress reporting entity
// TODO: named pipe entity (+ entity to introduce the contents of a named pipe as entities)

mod entity_ids;
mod entity_registry;
mod heartbeat;
mod scene_control;
mod timer;
mod logging;
mod properties;

pub use self::entity_ids::*;
pub use self::entity_registry::*;
pub use self::heartbeat::*;
pub use self::scene_control::*;
pub use self::timer::*;
pub use self::logging::*;
pub use self::properties::*;

// TODO: component to generate 'tick' messages whenever channels go from full to empty (and we're not processing a tick)
// TODO: component to generate timed events
// TODO: scripting component
// TODO: HTTP server component
// TODO: JSON streaming component
// TODO: named pipe component (+ component to introduce the contents of a named pipe as components)
// TODO: component to stop the scene (and other components?)

// TODO: component to contain properties/bindings (not really standard as we expect the user to create this)

mod entity_ids;
mod entity_registry;

pub use self::entity_ids::*;
pub use self::entity_registry::*;

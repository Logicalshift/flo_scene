//!
//! # Functions for passing messages to and from a 'guest context'
//!
//! A 'guest context' provides a way for group of subprograms to communicate with a scene using a set
//! of serialized messages. This is useful for creating components that run in a different environment.
//! Such environments can be things like remote processes connected via a socket, scripting languages, 
//! webassembly running locally or even in a user's browser. They are also a way of further isolating
//! a set of subprograms in a parent program.
//!
//! See the traits for a full list of things that need to be provided to create a guest (or a host). The
//! basics are that a guest needs a way to receive messages from the host, and to send messages back again;
//! it's effectively a slightly more involved version of the `poll` function from futures.
//!
//! Examples of where a guest might be used are for a wasm module or a subprogram that runs as an external
//! process.
//!

pub use flo_scene_guest::guest_types::*;
pub use flo_scene_guest::runtime::*;

mod guest_message_wrapper;
mod host_subprogram;
mod stream_target;

pub use guest_message_wrapper::*;
pub use host_subprogram::*;
pub use stream_target::*;

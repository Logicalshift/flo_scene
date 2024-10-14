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

mod traits;
mod guest_encoder;
mod poll_action;
mod poll_result;
mod runtime;
mod runtime_handle;
mod guest_context;
mod sink_handle;
mod stream_id;
mod stream_target;
mod subprogram_handle;
mod input_stream;
mod host_subprogram;

pub use traits::*;
pub use guest_encoder::*;
pub use poll_action::*;
pub use poll_result::*;
pub use runtime::*;
pub use runtime_handle::*;
pub use guest_context::*;
pub use sink_handle::*;
pub use stream_id::*;
pub use stream_target::*;
pub use subprogram_handle::*;
pub use input_stream::*;
pub use host_subprogram::*;

//!
//! # Functions for passing messages to and from a 'guest context'
//!
//! Normally scenes are run as a 'host context'. However, we may want to run subprograms in contexts
//! that are isolated from the 'host' scene: this API provides a means for a 'host' scene to communicate
//! with a 'guest' subprogram. Guests can be created almost completely isolated from their hosts.
//!
//! See the traits for a full list of things that need to be provided to create a guest (or a host). The
//! basics are that a guest needs a way to receive messages from the host, and to send messages back again;
//! it's effectively a slightly more involved version of the `poll` function from futures.
//!
//! Examples of where a guest might be used are for a wasm module or a subprogram that runs as an external
//! process.
//!

mod traits;
mod poll_action;
mod poll_result;
mod runtime;
mod sink_handle;
mod stream_id;
mod stream_target;
mod subprogram_handle;

pub use traits::*;
pub use poll_action::*;
pub use poll_result::*;
pub use runtime::*;
pub use sink_handle::*;
pub use stream_id::*;
pub use stream_target::*;
pub use subprogram_handle::*;

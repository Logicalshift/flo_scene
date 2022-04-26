//!
//! # Welcome to flo_scene
//!
//! `flo_scene` is a framework that can be used to compose small programs into larger programs, by structuring them
//! as entities that communicate by exchanging messages.
//!

mod error;
mod scene;
mod entity_id;
mod message;
mod entity_channel;

pub use self::error::*;
pub use self::scene::*;
pub use self::entity_id::*;
pub use self::message::*;
pub use self::entity_channel::*;

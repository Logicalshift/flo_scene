//!
//! # Welcome to flo_scene
//!
//! `flo_scene` is a framework that can be used to compose small programs into larger programs, by structuring them
//! as entities that communicate by exchanging messages.
//!

mod scene;
mod entity_id;
mod message;

pub use self::scene::*;
pub use self::entity_id::*;
pub use self::message::*;

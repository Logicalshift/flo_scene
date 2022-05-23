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
mod entity_channel_ext;
mod simple_entity_channel;
mod any_entity_channel;
mod mapped_entity_channel;
mod convert_entity_channel;
mod context;
mod stream_entity_response_style;
mod standard_components;

pub use self::error::*;
pub use self::scene::*;
pub use self::entity_id::*;
pub use self::message::*;
pub use self::entity_channel::*;
pub use self::entity_channel_ext::*;
pub use self::simple_entity_channel::*;
pub use self::mapped_entity_channel::*;
pub use self::convert_entity_channel::*;
pub use self::any_entity_channel::*;
pub use self::context::*;
pub use self::stream_entity_response_style::*;
pub use self::standard_components::*;

#[cfg(feature="test-scene")] pub mod test;

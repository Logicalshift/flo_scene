//!
//! # Welcome to flo_scene
//!
//! `flo_scene` is a framework that can be used to compose small programs into larger programs, by structuring them
//! as entities that communicate by exchanging messages.
//!

#[cfg(feature="properties")] #[macro_use] extern crate lazy_static;

mod error;
mod scene;
mod entity_id;
mod entity_channel;
mod immediate_entity_channel;
mod ergonomics;
mod simple_entity_channel;
mod any_entity_channel;
mod mapped_entity_channel;
mod convert_entity_channel;
mod expected_entity_channel;
mod context;
mod standard_components;
mod panic_entity_channel;

pub use self::error::*;
pub use self::scene::*;
pub use self::entity_id::*;
pub use self::entity_channel::*;
pub use self::immediate_entity_channel::*;
pub use self::ergonomics::*;
pub use self::simple_entity_channel::*;
pub use self::mapped_entity_channel::*;
pub use self::convert_entity_channel::*;
pub use self::panic_entity_channel::*;
pub use self::any_entity_channel::*;
pub use self::expected_entity_channel::*;
pub use self::context::*;
pub use self::standard_components::*;

#[cfg(feature="test-scene")] pub use self::ergonomics::test;
#[cfg(feature="properties")] pub use flo_binding as binding;
#[cfg(feature="properties")] pub use flo_rope as rope;

//!
//! # flo_scene_nostd
//!
//! `flo_scene` is a runtime system for Rust that provides a platform for building large software 
//! out of much smaller components. See the `flo_scene` crate for details.
//!
//! This provides a no_std implementation of the guest protocol used by flo_scene for dynamically
//! loaded components. This is particularly useful for implementing guest programs in wasm, as
//! this can produce considerably smaller assembly file sizes.
//!

#![no_std]

extern crate alloc;

pub mod exports;
pub mod imports;
mod guest_types;

pub use serde;
pub use postcard;
pub use futures;
pub use uuid;

pub use guest_types::*;

//!
//! # WASM module support for flo_scene
//!
//! This crate adds a subprogram for loading WASM modules into scenes.
//!
//! ## wasm32-unknown-unknown
//!
//! Standard modules can be compiled for the wasm32-unknown-unknown target. These can be made very
//! small, and will use the scene itself as the only means of communication with the rest of the world.
//!

//
// Notes on some decisions:
//
// I'm pretty new to wasmer as I'm writing this so there may be better approaches, but for v0.1 this
// is how things work:
//
// # wasm32-unknown-unknown with our own set of imports
//
// WASI may be a more flexible approach but it's quite complicated and seems to still be a bit
// immature at this point in time. With flo_scene, WASM modules can be quite useful even if all
// they can do is pass messages around and not having a bunch of extra runtime functions makes
// everything a bit more compact.
//
// wasm-bindgen is not usable here as it generates javascript bindings and we aren't running
// javascript :-)
//
// # Sending/receiving data
//
// This is a little annoying because we need to allocate buffers in wasm, then fill them, then
// do the same in reverse in order to read from the wasm vm. For fixed-sized structs the values
// can be sent in as parameters to a function, but for things like vecs or boxed items memory
// space is required.
//
// (There is a matching possibility of using multi-return functions for decoding structures but
// these have the downside of requiring a special compiler flag rather than a function attribute
// to turn on)
//
// The easiest method might be serde. This does have some downsides: it's not possible to
// pre-allocate the space, for one thing, and unless we're using something like JSON we still
// need to implement serializers for all the types, which is not much easier than writing
// a macro anyway. Another thing that can't really be done with serde is having a way to 
// check the type signature.
//
// We also need a way to send types between two subprograms in webassembly modules that
// don't have equivalent types on the Rust side: we can send the 'encoded' form in order
// to achieve this.
//
// So the plan is to use a macro to convert the types to/from webassembly.
//
// # Running the futures
//
// There's no real way to pass a future directly from webassembly, so we need another macro
// to convert to a type we can call from the host side. We'll also need to run in our own 
// context inside webassembly. All of the streams and other things are managed on the host
// side so the webassembly context should be able to be pretty simple.
//
// # Types
//
// On the webassembly side, we need to have equivalents to `InputStream`, `SceneContext`
// and `OutputSink`. They just retrieve references to things on the host side rather than
// being the full implementation, but they do need to replicate the functions that are
// available on the host side.
//
// # Subprograms
//
// Finding the list of subprograms in a webassembly module is a bit tricky. The `inventory`
// crate doesn't work for webassembly, but we can iterate over the exports in wasmer to find
// functions that match a particular name/type signature.
//

mod control_subprogram;
mod error;
mod module;
mod module_id;
mod wasm_control;

pub use control_subprogram::*;
pub use error::*;
pub use module_id::*;
pub use wasm_control::*;

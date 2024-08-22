//!
//! Raw test: this builds with `cargo build --target wasm32-unknown-unknown` and needs very few imports
//!
//! This gives us very light modules that aren't very capable, but if we're adding an ABI to communicate with flo_scene anyway
//! none of this matters, as these modules should only use the scene for communicating with the outside world. They're very
//! simple to load and run with wasmer, but probably need a bunch of hand-holding to work.
//!
//! These modules should work just as well in a browser as in wasmer.
//!

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn test() -> u32 {
    let foo = vec![1, 2, 3, 42];

    foo[3]
}

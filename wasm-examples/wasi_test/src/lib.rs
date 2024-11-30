//!
//! This uses WASI, which is much more heavyweight than the raw example. We can talk native functions in here, but WASI is
//! still in development and needs things like .WIT files in order to work. WASI assumes you're writing specific types of
//! components and has an involved interop layer which is potentially not very useful for scenery items which just need to
//! talk to flo_scene.
//!
//! Compile with `cargo build --target wasm32-wasip1` to get something that works (we're sort of taking advantage that the
//! wasm32-unknown-unknown target also works here to make the raw example easily buildbale, I'm not sure if that works or 
//! works with weird issues or what: not actually running it)
//!

use flo_scene::*;
use flo_scene::host::programs::*;

use futures::prelude::*;

wit_bindgen::generate!({
    world: "host",
});

struct TestHost;

impl Guest for TestHost {
    fn test() -> u32 {
        42
    }
}

export!(TestHost);

pub async fn test() -> i32 {
    future::ready(42).await
}

/*
#[wasm_bindgen]
pub async fn test(input: InputStream<String>, context: SceneContext) {
    let mut input = input;

    while let Some(msg) = input.next().await {
        context.send_message(TextOutput::Line(msg)).await;
    }
}
*/
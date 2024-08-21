use flo_scene::*;
use flo_scene::programs::*;

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
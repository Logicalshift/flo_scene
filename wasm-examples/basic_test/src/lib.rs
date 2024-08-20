use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
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
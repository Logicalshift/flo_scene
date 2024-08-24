use super::wasm_control::*;

use flo_scene::*;

use futures::prelude::*;

///
/// A subprogram that loads and runs subprograms written in WASM
///
pub async fn wasm_control_subprogram(input: InputStream<WasmControl>, context: SceneContext) {
    let mut input = input;

    while let Some(instruction) = input.next().await {
        use WasmControl::*;

        match instruction {
            LoadModule(module_id, module_bytes, update_target)  => { }
            RunModule(module_id, program_id)                    => { }
        }
    }
}

#!/bin/sh

cargo clean
cargo build --target wasm32-unknown-unknown --release

wasm-opt -Oz -o wasm/flo_scene_wasm_buffer_tests.wasm target/wasm32-unknown-unknown/release/flo_scene_wasm_buffer_tests.wasm

# cp target/wasm32-unknown-unknown/release/*.wasm ./wasm

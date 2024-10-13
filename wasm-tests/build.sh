#!/bin/sh

cargo clean
cargo build --target wasm32-unknown-unknown --release

cp target/wasm32-unknown-unknown/release/*.wasm ./wasm

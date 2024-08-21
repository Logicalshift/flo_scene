use wasmer::*;

fn main() {
    let module = include_bytes!("../../wasm-examples/target/wasm32-wasip1/debug/flo_scene_wasm_wasi_test.wasm");

    let mut store       = Store::default();
    let module          = Module::new(&store, &module).unwrap();
    let import_object   = imports! {};
    let instance        = Instance::new(&mut store, &module, &import_object).unwrap();

    let test            = instance.exports.get_function("test").unwrap();
    println!("Test type: {:?}", test.ty(&store));
}

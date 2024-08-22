use wasmer::*;

fn main() {
    let module = include_bytes!("../../wasm-examples/target/wasm32-unknown-unknown/debug/flo_scene_wasm_raw_test.wasm");

    let mut store       = Store::default();
    let module          = Module::new(&store, &module).unwrap();
    let import_object   = imports! {};
    let instance        = Instance::new(&mut store, &module, &import_object).unwrap();

    let test            = instance.exports.get_function("test").unwrap();
    let test2           = instance.exports.get_function("test2").unwrap();
    let test3           = instance.exports.get_function("test3").unwrap();
    println!("Test type: {:?}", test.ty(&store));
    println!("Test type 2: {:?}", test2.ty(&store));
    println!("Test type 3: {:?}", test3.ty(&store));

    let result = test.call(&mut store, &[]);
    println!("1 {:?}", result);

    let result = test2.call(&mut store, &[]);
    println!("2 {:?}", result);

    let result = test3.call(&mut store, &[]);
    println!("3 {:?}", result);
}

use std::borrow::Borrow;

use wasmer::*;

use std::mem;

// Matches what's in the wasm
#[repr(C)]
#[derive(Debug)]
pub enum Foo {
    Foo(f32),
    Bar,
    Baz(u128),
}

#[repr(C)]
#[derive(Debug)]
pub struct Bar {
    val1: i32,
    val2: f32,
}

fn main() {
    let module = include_bytes!("../../wasm-examples/target/wasm32-unknown-unknown/debug/flo_scene_wasm_raw_test.wasm");

    let mut store       = Store::default();
    let module          = Module::new(&store, &module).unwrap();
    let import_object   = imports! {};
    let instance        = Instance::new(&mut store, &module, &import_object).unwrap();

    let test            = instance.exports.get_function("test").unwrap();
    let test2           = instance.exports.get_function("test2").unwrap();
    let test3           = instance.exports.get_function("test3").unwrap();
    let test4           = instance.exports.get_function("test4").unwrap();
    let test7           = instance.exports.get_function("test7").unwrap();
    println!("Test type: {:?}", test.ty(&store));
    println!("Test type 2: {:?}", test2.ty(&store));
    println!("Test type 3: {:?}", test3.ty(&store));
    println!("Test type 4: {:?}", test4.ty(&store));
    println!("Test type 7: {:?}", test7.ty(&store));

    let result = test.call(&mut store, &[]);
    println!("1 {:?}", result);

    let result = test2.call(&mut store, &[]);
    println!("2 {:?}", result);

    let result = test3.call(&mut store, &[]);
    println!("3 {:?}", result);

    let result4 = test4.call(&mut store, &[]);
    println!("4 {:?}", result);

    let memory          = instance.exports.get_memory("memory").unwrap();
    let view            = memory.view(&store);
    let mut bar_bytes   = [0u8; size_of::<Bar>()];
    view.read(match &result4.unwrap()[0] { Value::I32(offset) => *offset as u64, _ => panic!() }, &mut bar_bytes).unwrap();
    println!("Read {:?}", bar_bytes);

    let actually_bar: Bar = unsafe { mem::transmute(bar_bytes) };
    println!("Transmuted: {:?}", actually_bar);

    let test5           = instance.exports.get_function("test5").unwrap();
    let result5         = test5.call(&mut store, &[]);
    let memory          = instance.exports.get_memory("memory").unwrap();
    let view            = memory.view(&store);
    let mut foo_bytes   = [0u8; size_of::<Foo>()];

    view.read(match &result5.unwrap()[0] { Value::I32(offset) => *offset as u64, _ => panic!() }, &mut foo_bytes).unwrap();
    println!("Read {:?}", foo_bytes);

    let actually_foo: Foo = unsafe { mem::transmute(foo_bytes) };
    println!("Transmuted: {:?}", actually_foo);
}

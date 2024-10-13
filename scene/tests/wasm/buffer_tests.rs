use wasmer::*;

/// Bytecode for the tests
static BUFFER_TESTS_WASM: &'static [u8] = include_bytes!("../../../wasm-tests/wasm/flo_scene_wasm_buffer_tests.wasm");

#[test]
pub fn load_module() {
    // Load the buffer module
    let mut store       = Store::default();
    let module          = Module::new(&store, &*BUFFER_TESTS_WASM).unwrap();
    let imports         = imports! { };
    Instance::new(&mut store, &module, &imports).unwrap();
}

#[test]
pub fn borrow_buffer() {
    // Load the buffer module
    let mut store       = Store::default();
    let module          = Module::new(&store, &*BUFFER_TESTS_WASM).unwrap();
    let imports         = imports! { };
    let buffer_tests    = Instance::new(&mut store, &module, &imports).unwrap();

    // Borrow buffer 1
    let borrow_buffer   = buffer_tests.exports.get_function("scene_borrow_buffer").unwrap();
    let buffer_address  = borrow_buffer.call(&mut store, &[Value::I32(1), Value::I32(4)]).unwrap();

    println!("Buffer address is {:?}", buffer_address);

    buffer_tests.exports.get_memory("memory").unwrap();
}

#[test]
pub fn read_buffer_set_correctly() {
    // Load the buffer module
    let mut store       = Store::default();
    let module          = Module::new(&store, &*BUFFER_TESTS_WASM).unwrap();
    let imports         = imports! { };
    let buffer_tests    = Instance::new(&mut store, &module, &imports).unwrap();
    let memory          = buffer_tests.exports.get_memory("memory").unwrap();

    // Borrow buffer 1
    let borrow_buffer   = buffer_tests.exports.get_function("scene_borrow_buffer").unwrap();
    let buffer_address  = borrow_buffer.call(&mut store, &[Value::I32(1), Value::I32(4)]).unwrap();

    // Write 1 2 3 4 to it
    let view = memory.view(&store);
    view.write(buffer_address[0].unwrap_i32() as _, &[1, 2, 3, 4]).unwrap();

    // Test that the correct value was written
    let buffer_contents_are_1234    = buffer_tests.exports.get_function("buffer_contents_are_1234").unwrap();
    let was_written                 = buffer_contents_are_1234.call(&mut store, &[]).unwrap();

    assert!(was_written[0].unwrap_i32() != 0, "{:?}", was_written);
}

#[test]
pub fn read_buffer_set_incorrectly() {
    // Load the buffer module
    let mut store       = Store::default();
    let module          = Module::new(&store, &*BUFFER_TESTS_WASM).unwrap();
    let imports         = imports! { };
    let buffer_tests    = Instance::new(&mut store, &module, &imports).unwrap();
    let memory          = buffer_tests.exports.get_memory("memory").unwrap();

    // Borrow buffer 1
    let borrow_buffer   = buffer_tests.exports.get_function("scene_borrow_buffer").unwrap();
    let buffer_address  = borrow_buffer.call(&mut store, &[Value::I32(1), Value::I32(4)]).unwrap();

    // Write 5, 6, 7, 8 to it (setting it to the incorrect values)
    let view = memory.view(&store);
    view.write(buffer_address[0].unwrap_i32() as _, &[5, 6, 7, 8]).unwrap();

    // Test that the correct value was written
    let buffer_contents_are_1234    = buffer_tests.exports.get_function("buffer_contents_are_1234").unwrap();
    let was_written                 = buffer_contents_are_1234.call(&mut store, &[]).unwrap();

    assert!(was_written[0].unwrap_i32() == 0, "{:?}", was_written);
}

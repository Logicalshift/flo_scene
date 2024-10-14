use crate::error::*;

use flo_scene::guest::*;

use wasmer::*;

///
/// Functions for manipulating buffers on the WASM guest side
///
pub struct BufferFunctions {
    new_buffer:     TypedFunction<(), i32>,
    borrow_buffer:  TypedFunction<(i32, i32), i32>,
    buffer_size:    TypedFunction<i32, i32>,
    free_buffer:    TypedFunction<i32, ()>,
}

///
/// Set of runtime functions (these are the same set for different encodings, postcard encoding is always supported)
///
pub struct RuntimeFunctions {
    send_message:           TypedFunction<(i32, i32, i32), ()>,
    sink_ready:             TypedFunction<(i32, i32), ()>,
    sink_connection_error:  TypedFunction<(i32, i32, i32), ()>,
    sink_send_error:        TypedFunction<(i32, i32, i32), ()>,
    poll_awake:             TypedFunction<i32, i32>,
}

///
/// A WASM module loaded by the control subprogram
///
pub struct WasmModule {
    store:      Store,
    module:     Module,
    instance:   Instance,
    memory:     Memory,

    buffer:     BufferFunctions,
    runtime:    RuntimeFunctions,
}

impl WasmModule {
    ///
    /// Loads a 'bare' module with the default runtime
    ///
    pub fn load_bare_module(module_bytes: &[u8]) -> Result<Self, WasmSubprogramError> {
        let mut store   = Store::default();
        let module      = Module::new(&store, &module_bytes)?;
        let imports     = Self::bare_imports();
        let instance    = Instance::new(&mut store, &module, &imports)?;
        let memory      = instance.exports.get_memory("memory").unwrap().clone();

        let buffer      = BufferFunctions::from_instance(&instance, &mut store)?;
        let runtime     = RuntimeFunctions::from_instance(&instance, &mut store, "postcard")?;

        Ok(WasmModule { store, module, instance, memory, buffer, runtime })
    }

    ///
    /// The default set of imports for a 'bare' module
    ///
    fn bare_imports() -> Imports {
        imports! { }
    }

    ///
    /// Copies a buffer to the wasm side, and returns the buffer handle
    ///
    fn copy_buffer(&mut self, data: Vec<u8>) -> i32 {
        let buffer  = &self.buffer;
        let memory  = &self.memory;
        let store   = &mut self.store;

        // Create a new buffer and borrow it
        let buffer_handle   = buffer.new_buffer.call(store).unwrap();
        let buffer_data_ptr = buffer.borrow_buffer.call(store, buffer_handle, data.len() as _).unwrap();

        // Copy the data to the buffer
        let view = memory.view(&store);
        view.write(buffer_data_ptr as _, &data).unwrap();

        buffer_handle
    }

    ///
    /// Reads a buffer from the wasm
    ///
    fn read_buffer(&mut self, buffer_handle: i32) -> Vec<u8> {
        todo!()
    }
}

impl BufferFunctions {
    ///
    /// Imports the buffer functions from the specified instance
    ///
    pub fn from_instance(instance: &Instance, store: &mut Store) -> Result<BufferFunctions, WasmSubprogramError> {
        let new_buffer      = instance.exports.get_function("scene_new_buffer").map_err(|_| WasmSubprogramError::MissingBufferFunction("scene_new_buffer".into()))?.typed(store).unwrap();
        let borrow_buffer   = instance.exports.get_function("scene_borrow_buffer").map_err(|_| WasmSubprogramError::MissingBufferFunction("scene_borrow_buffer".into()))?.typed(store).unwrap();
        let buffer_size     = instance.exports.get_function("scene_buffer_size").map_err(|_| WasmSubprogramError::MissingBufferFunction("scene_buffer_size".into()))?.typed(store).unwrap();
        let free_buffer     = instance.exports.get_function("scene_free_buffer").map_err(|_| WasmSubprogramError::MissingBufferFunction("scene_free_buffer".into()))?.typed(store).unwrap();

        Ok(BufferFunctions { new_buffer, borrow_buffer, buffer_size, free_buffer })
    }
}

impl RuntimeFunctions {
    ///
    /// Imports the runtime functions from the specified instance, for the specified serialization format
    ///
    pub fn from_instance(instance: &Instance, store: &mut Store, serialization_format: &str) -> Result<RuntimeFunctions, WasmSubprogramError> {
        let send_message            = instance.exports.get_function(&format!("scene_guest_{}_send_message", serialization_format)).map_err(|_| WasmSubprogramError::MissingRuntimeFunction(format!("scene_guest_{}_send_message", serialization_format)))?.typed(store).unwrap();
        let sink_ready              = instance.exports.get_function(&format!("scene_guest_{}_sink_ready", serialization_format)).map_err(|_| WasmSubprogramError::MissingRuntimeFunction(format!("scene_guest_{}_sink_ready", serialization_format)))?.typed(store).unwrap();
        let sink_connection_error   = instance.exports.get_function(&format!("scene_guest_{}_sink_connection_error", serialization_format)).map_err(|_| WasmSubprogramError::MissingRuntimeFunction(format!("scene_guest_{}_sink_connection_error", serialization_format)))?.typed(store).unwrap();
        let sink_send_error         = instance.exports.get_function(&format!("scene_guest_{}_sink_send_error", serialization_format)).map_err(|_| WasmSubprogramError::MissingRuntimeFunction(format!("scene_guest_{}_sink_send_error", serialization_format)))?.typed(store).unwrap();
        let poll_awake              = instance.exports.get_function(&format!("scene_guest_{}_poll_awake", serialization_format)).map_err(|_| WasmSubprogramError::MissingRuntimeFunction(format!("scene_guest_{}_poll_awake", serialization_format)))?.typed(store).unwrap();

        Ok(RuntimeFunctions { 
            send_message,
            sink_ready,
            sink_connection_error,
            sink_send_error,
            poll_awake
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    static BUFFER_TESTS_WASM: &'static [u8] = include_bytes!("../../wasm-tests/wasm/flo_scene_wasm_buffer_tests.wasm");

    #[test]
    fn load_buffer_tests() {
        // The buffer tests are linked against flo_scene so should load successfully as a module
        let module = WasmModule::load_bare_module(&BUFFER_TESTS_WASM);

        assert!(module.is_ok(), "{:?}", module.err());
    }

    #[test]
    fn copy_buffer() {
        // The buffer tests are linked against flo_scene so should load successfully as a module
        let mut module = WasmModule::load_bare_module(&BUFFER_TESTS_WASM).unwrap();

        module.copy_buffer(vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
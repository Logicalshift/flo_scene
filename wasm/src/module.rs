use crate::error::*;

use flo_scene::guest::*;

use postcard;
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
        let view = memory.view(store);
        view.write(buffer_data_ptr as _, &data).unwrap();

        buffer_handle
    }

    ///
    /// Receives a buffer from the wasm
    ///
    fn receive_buffer(&mut self, buffer_handle: i32) -> Vec<u8> {
        let buffer  = &self.buffer;
        let memory  = &self.memory;
        let store   = &mut self.store;

        let buffer_size = buffer.buffer_size.call(store, buffer_handle).unwrap();

        let result = if buffer_size > 0 {
            // Borrow the buffer
            let buffer_data_ptr = buffer.borrow_buffer.call(store, buffer_handle, buffer_size).unwrap();

            // Read the bytes from memory into a new vec
            let buffer_size = buffer_size as usize;
            let view        = memory.view(&store);
            let mut result  = vec![0; buffer_size];

            view.read(buffer_data_ptr as _, &mut result).unwrap();

            result
        } else {
            vec![]
        };

        // Release the buffer after 
        buffer.free_buffer.call(store, buffer_handle).unwrap();

        result
    }

    ///
    /// Sends a message to the runtime in the wasm module
    ///
    pub fn send_message(&mut self, runtime: GuestRuntimeHandle, target: GuestSubProgramHandle, data: Vec<u8>) {
        // Send the data to the target
        let data_handle = self.copy_buffer(data);

        // Convert the runtime and target IDs to i32s
        let runtime_id = runtime.0 as i32;
        let target_id  = target.0 as i32;

        // Tell the runtime to send the message
        let store   = &mut self.store;
        let runtime = &self.runtime;
        runtime.send_message.call(store, runtime_id, target_id, data_handle).unwrap();
    }

    ///
    /// Indicates that a sink is ready to the runtime
    ///
    pub fn sink_ready(&mut self, runtime: GuestRuntimeHandle, sink: HostSinkHandle) {
        // Convert the runtime and target IDs to i32s
        let runtime_id = runtime.0 as i32;
        let sink_id    = sink.0 as i32;

        // Tell the runtime that the sink is ready
        let store   = &mut self.store;
        let runtime = &self.runtime;
        runtime.sink_ready.call(store, runtime_id, sink_id).unwrap();
    }

    ///
    /// Indicates that an error occurred while connecting a sink
    ///
    pub fn sink_connection_error(&mut self, runtime: GuestRuntimeHandle, sink: HostSinkHandle, error: Vec<u8>) {
        // Send the data to the target
        let error_handle = self.copy_buffer(error);

        // Convert the runtime and target IDs to i32s
        let runtime_id = runtime.0 as i32;
        let sink_id    = sink.0 as i32;

        // Tell the runtime that the sink has a connection error
        let store   = &mut self.store;
        let runtime = &self.runtime;
        runtime.sink_connection_error.call(store, runtime_id, sink_id, error_handle).unwrap();
    }

    ///
    /// Indicates that an error occurred while sending to a sink
    ///
    pub fn sink_send_error(&mut self, runtime: GuestRuntimeHandle, sink: HostSinkHandle, error: Vec<u8>) {
        // Send the data to the target
        let error_handle = self.copy_buffer(error);

        // Convert the runtime and target IDs to i32s
        let runtime_id = runtime.0 as i32;
        let sink_id    = sink.0 as i32;

        // Tell the runtime that the sink has a send error
        let store   = &mut self.store;
        let runtime = &self.runtime;
        runtime.sink_send_error.call(store, runtime_id, sink_id, error_handle).unwrap();
    }

    ///
    /// Polls the runtime, and returns the actions to perform on the host side
    ///
    /// This needs to be called after sending any of the other actions as this will actually 
    ///
    pub fn poll_awake(&mut self, runtime: GuestRuntimeHandle) -> GuestResult {
        let runtime_id = runtime.0 as i32;

        // Poll the runtime
        let store           = &mut self.store;
        let runtime         = &self.runtime;
        let result_buffer   = runtime.poll_awake.call(store, runtime_id).unwrap();

        // Result will always use the postcard encoding (but may have messages wrapped in another encoding)
        let result_buffer = self.receive_buffer(result_buffer);
        postcard::from_bytes(&result_buffer).unwrap()
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

    #[test]
    fn receive_buffer() {
        // The buffer tests are linked against flo_scene so should load successfully as a module
        let mut module = WasmModule::load_bare_module(&BUFFER_TESTS_WASM).unwrap();

        let handle      = module.copy_buffer(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let contents    = module.receive_buffer(handle);

        assert!(contents == vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}

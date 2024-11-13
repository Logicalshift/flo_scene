use crate::error::*;

use flo_scene::{SubProgramId, SerializationId};
use flo_scene::guest::*;

use postcard;
use wasmer::*;
use uuid::{Uuid};

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
    send_stream:            TypedFunction<(i32, i32, i32), ()>,
    ready_stream:           TypedFunction<(i32, i32), ()>,
    close_stream:           TypedFunction<(i32, i32), ()>,
}

///
/// Environment passed in to functions (mainly used to make the memory accessible)
///
/// This isn't very well documented by wasmer itself: you can't get access to the memory from the instance when declaring the imports
/// because the imports are needed to create the instance, so we need a way to set the memory later on, and environments are the 
/// mechanism that's provided for this purpose.
///
struct ModuleEnvironment {
    /// The memory declared by the wasm program
    memory: Option<Memory>
}

///
/// A WASM module loaded by the control subprogram
///
pub struct WasmModule {
    store:      Store,
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
        let environment = FunctionEnv::new(&mut store, ModuleEnvironment { memory: None });
        let module      = Module::new(&store, &module_bytes)?;
        let imports     = Self::bare_imports(&mut store, &environment);
        let instance    = Instance::new(&mut store, &module, &imports)?;
        let memory      = instance.exports.get_memory("memory").unwrap().clone();

        environment.as_mut(&mut store).memory = Some(memory.clone());

        let buffer      = BufferFunctions::from_instance(&instance, &mut store)?;
        let runtime     = RuntimeFunctions::from_instance(&instance, &mut store, "postcard")?;

        Ok(WasmModule { store, instance, memory, buffer, runtime })
    }

    ///
    /// The default set of imports for a 'bare' module
    ///
    fn bare_imports(store: &mut Store, environment: &FunctionEnv<ModuleEnvironment>) -> Imports {
        imports! {
            "env" => {
                // There's no RNG available in WASM so we 
                "scene_request_new_uuid" => Function::new_typed_with_env(store, environment, |env: FunctionEnvMut<ModuleEnvironment>, uid_adr: i32| {
                    // Create a new v4 UUID and convert to bytes
                    let uuid        = Uuid::new_v4();
                    let uuid_bytes  = uuid.as_bytes();

                    // Copy the bytes into the WASM memory (the environment is used to pass the memory in)
                    let mut env         = env;
                    let (env, store)    = env.data_and_store_mut();
                    let view            = env.memory.as_ref().unwrap().view(&store);

                    view.write(uid_adr as _, uuid_bytes).unwrap();
                }),
            }
        }
    }

    ///
    /// Copies a buffer to the wasm side, and returns the buffer handle
    ///
    /// On the webassembly side, this creates a buffer that can be retrieved using `claim_buffer` (so the host side is not
    /// responsible for releasing it)
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
    pub fn poll_awake(&mut self, runtime: GuestRuntimeHandle) -> Vec<GuestResult> {
        let runtime_id = runtime.0 as i32;

        // Poll the runtime
        let store           = &mut self.store;
        let runtime         = &self.runtime;
        let result_buffer   = runtime.poll_awake.call(store, runtime_id).unwrap();

        // Result will always use the postcard encoding (but may have messages wrapped in another encoding)
        let result_buffer = self.receive_buffer(result_buffer);
        postcard::from_bytes(&result_buffer).unwrap()
    }

    ///
    /// Converts a serialization ID to an i32 value
    ///
    #[inline]
    fn serialization_id_to_i32(id: SerializationId) -> i32 {
        match id {
            SerializationId::SimpleStream(id) => {
                id as i32
            },

            SerializationId::SimpleFunction(id) => {
                -(id as i32) - 1
            }
        }
    }

    ///
    /// Sends a message to a stream
    ///
    pub fn send_stream(&mut self, runtime: GuestRuntimeHandle, stream_id: SerializationId, msg: Vec<u8>) {
        // Send the message to a buffer on the wasm side
        let runtime_id  = runtime.0 as i32;
        let stream_id   = Self::serialization_id_to_i32(stream_id);
        let data_handle = self.copy_buffer(msg);

        // Finish sending the data
        let store       = &mut self.store;
        let runtime     = &self.runtime;
        runtime.send_stream.call(store, runtime_id, stream_id, data_handle).unwrap();
    }

    ///
    /// Indicates to the guest that a host stream is ready for more data
    ///
    pub fn ready_stream(&mut self, runtime: GuestRuntimeHandle, stream_id: SerializationId) {
        let runtime_id  = runtime.0 as i32;
        let stream_id   = Self::serialization_id_to_i32(stream_id);

        let store       = &mut self.store;
        let runtime     = &self.runtime;
        runtime.ready_stream.call(store, runtime_id, stream_id).unwrap();
    }

    ///
    /// Indicates to the guest that a stream has been closed
    ///
    pub fn close_stream(&mut self, runtime: GuestRuntimeHandle, stream_id: SerializationId) {
        let runtime_id  = runtime.0 as i32;
        let stream_id   = Self::serialization_id_to_i32(stream_id);

        let store       = &mut self.store;
        let runtime     = &self.runtime;
        runtime.close_stream.call(store, runtime_id, stream_id).unwrap();
    }

    ///
    /// Processes a single action in this runtime (note that `poll_awake()` needs to be called after this to actually execute the runtime)
    ///
    pub fn process(&mut self, runtime: GuestRuntimeHandle, action: GuestAction) {
        use GuestAction::*;

        match action {
            SendMessage(sub_program, message)       => { self.send_message(runtime, sub_program, message) }
            Ready(sink_handle)                      => { self.sink_ready(runtime, sink_handle) },
            SinkConnectionError(sink_handle, error) => { self.sink_connection_error(runtime, sink_handle, postcard::to_stdvec(&error).unwrap()) },
            SinkError(sink_handle, error)           => { self.sink_send_error(runtime, sink_handle, postcard::to_stdvec(&error).unwrap()) }
            SendStream(stream_id, msg)              => { self.send_stream(runtime, stream_id, msg) },
            ReadyStream(stream_id)                  => { self.ready_stream(runtime, stream_id) },
            CloseStream(stream_id)                  => { self.close_stream(runtime, stream_id) },
        }
    }

    ///
    /// Starts a subprogram from the wasm module
    ///
    /// Internally, the handle is generated by converting the program ID to a string and running the `start_ID_program()` function,
    /// which should return a runtime handle
    ///
    pub fn start_guest(&mut self, program: SubProgramId) -> Result<GuestRuntimeHandle, WasmSubprogramError> {
        let instance    = &self.instance;
        let store       = &mut self.store;

        let program_name    = program.to_string();
        let function_name   = format!("start_{}_subprogram", program_name);

        let start_function  = instance.exports.get_typed_function::<(), i32>(store, &function_name).map_err(|_| WasmSubprogramError::MissingStartFunction(function_name.clone()))?;
        let guest_runtime   = start_function.call(store).map_err(|err| WasmSubprogramError::CouldNotCallStartFunction(function_name.clone(), format!("{:?}", err)))?;

        Ok(GuestRuntimeHandle(guest_runtime as _))
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
        let send_stream             = instance.exports.get_function(&format!("scene_guest_{}_send_stream", serialization_format)).map_err(|_| WasmSubprogramError::MissingRuntimeFunction(format!("scene_guest_{}_poll_awake", serialization_format)))?.typed(store).unwrap();
        let ready_stream            = instance.exports.get_function(&format!("scene_guest_{}_ready_stream", serialization_format)).map_err(|_| WasmSubprogramError::MissingRuntimeFunction(format!("scene_guest_{}_poll_awake", serialization_format)))?.typed(store).unwrap();
        let close_stream            = instance.exports.get_function(&format!("scene_guest_{}_close_stream", serialization_format)).map_err(|_| WasmSubprogramError::MissingRuntimeFunction(format!("scene_guest_{}_poll_awake", serialization_format)))?.typed(store).unwrap();

        Ok(RuntimeFunctions { 
            send_message,
            sink_ready,
            sink_connection_error,
            sink_send_error,
            poll_awake,
            send_stream,
            ready_stream,
            close_stream,
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

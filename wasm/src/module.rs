use crate::error::*;

use wasmer::*;

///
/// Functions for manipulating buffers on the WASM guest side
///
pub struct BufferFunctions {
    new_buffer:       Function,
    borrow_buffer:    Function,
    free_buffer:      Function,
}

///
/// Set of runtime functions (these are the same set for different encodings, postcard encoding is always supported)
///
pub struct RuntimeFunctions {
    send_message:           Function,
    sink_ready:             Function,
    sink_connection_error:  Function,
    sink_send_error:        Function,
    poll_awake:             Function,
}

///
/// A WASM module loaded by the control subprogram
///
pub struct WasmModule {
    store:      Store,
    module:     Module,
    instance:   Instance,

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

        let buffer      = BufferFunctions::from_instance(&instance).ok_or(WasmSubprogramError::MissingBufferFunctions)?;
        let runtime     = RuntimeFunctions::from_instance(&instance, "postcard").ok_or(WasmSubprogramError::MissingRuntimeFunctions)?;

        Ok(WasmModule { store, module, instance, buffer, runtime })
    }

    ///
    /// The default set of imports for a 'bare' module
    ///
    fn bare_imports() -> Imports {
        imports! { }
    }
}

impl BufferFunctions {
    ///
    /// Imports the buffer functions from the specified instance
    ///
    pub fn from_instance(instance: &Instance) -> Option<BufferFunctions> {
        let new_buffer      = instance.exports.get_function("scene_new_buffer").ok()?.clone();
        let borrow_buffer   = instance.exports.get_function("scene_borrow_buffer").ok()?.clone();
        let free_buffer     = instance.exports.get_function("scene_free_buffer").ok()?.clone();

        Some(BufferFunctions { new_buffer, borrow_buffer, free_buffer })
    }
}

impl RuntimeFunctions {
    ///
    /// Imports the runtime functions from the specified instance, for the specified serialization format
    ///
    pub fn from_instance(instance: &Instance, serialization_format: &str) -> Option<RuntimeFunctions> {
        let send_message            = instance.exports.get_function(&format!("scene_guest_{}_send_message", serialization_format)).ok()?.clone();
        let sink_ready              = instance.exports.get_function(&format!("scene_guest_{}_sink_ready", serialization_format)).ok()?.clone();
        let sink_connection_error   = instance.exports.get_function(&format!("scene_guest_{}_sink_connection_error", serialization_format)).ok()?.clone();
        let sink_send_error         = instance.exports.get_function(&format!("scene_guest_{}_sink_send_error", serialization_format)).ok()?.clone();
        let poll_awake              = instance.exports.get_function(&format!("scene_guest_{}_poll_awake", serialization_format)).ok()?.clone();

        Some(RuntimeFunctions { 
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
}
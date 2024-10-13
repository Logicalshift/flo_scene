use std::sync::atomic::{AtomicUsize, Ordering};

///
/// The guest runtime handle is used by the host side to make requests to a runtime defined on the wasm side.
/// There can be more than one runtime if needed, though most scenarios can be executed using just a single
/// runtime. Runtimes can only use one message encoding strategy, so one reason that multiple might be used
/// is that multiple strategies are in use.
///
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
#[repr(transparent)]
pub struct GuestRuntimeHandle(pub usize);

///
/// Assigns a new guest runtime handle
///
pub (super) fn allocate_handle() -> GuestRuntimeHandle {
    // The next handle to assign
    static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(0);

    let this_handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    GuestRuntimeHandle(this_handle)
}

#[cfg(feature="serde_json")]
pub use json_runtime::*;

#[cfg(feature="serde_json")]
mod json_runtime {
    use super::*;
    use crate::guest::*;
    use crate::wasm_rt::buffer::*;

    use once_cell::sync::{Lazy};

    use std::collections::{HashMap};
    use std::sync::*;

    /// Guest runtimes using the JSON encoding
    static GUEST_JSON_RUNTIMES: Lazy<Mutex<HashMap<GuestRuntimeHandle, Arc<GuestRuntime<GuestJsonEncoder>>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

    ///
    /// Registers a guest runtime and returns the handle which can be passed on to the host side of things
    ///
    pub fn register_json_runtime(new_runtime: GuestRuntime<GuestJsonEncoder>) -> GuestRuntimeHandle {
        // Assign a handle and store in the guest list
        let handle = allocate_handle();
        GUEST_JSON_RUNTIMES.lock().unwrap().insert(handle, Arc::new(new_runtime));

        handle
    }

    ///
    /// Sends a message to a guest subprogram in a runtime
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_json_send_message(runtime: GuestRuntimeHandle, target: GuestSubProgramHandle, json_data: BufferHandle) {
        // Get the JSON runtime with this ID
        let runtime = GUEST_JSON_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();

        // Retrieve the JSON data buffer from where it was being written by the host
        let json_data = claim_buffer(json_data);

        // Send the message to the runtime
        runtime.send_message(target, json_data);
    }

    ///
    /// Indicates to a guest subprogram that it is safe to send to a sink
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_json_sink_ready(runtime: GuestRuntimeHandle, sink: HostSinkHandle) {
        let runtime = GUEST_JSON_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();
        runtime.sink_ready(sink);
    }

    ///
    /// Indicates to aguest subprogram that an error ocurred while connecting a sink
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_json_sink_connection_error(runtime: GuestRuntimeHandle, sink: HostSinkHandle, json_error: BufferHandle) {
        let json_error  = claim_buffer(json_error);
        let error       = serde_json::from_slice(&json_error).unwrap();

        let runtime = GUEST_JSON_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();
        runtime.sink_connection_error(sink, error);
    }

    ///
    /// Indicates to a guest subprogram that an error ocurred while sending data to a sink
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_json_sink_send_error(runtime: GuestRuntimeHandle, sink: HostSinkHandle, json_error: BufferHandle) {
        let json_error  = claim_buffer(json_error);
        let error       = serde_json::from_slice(&json_error).unwrap();

        let runtime = GUEST_JSON_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();
        runtime.sink_send_error(sink, error);
    }
}

#[cfg(feature="postcard")]
pub use postcard_runtime::*;

#[cfg(feature="postcard")]
mod postcard_runtime {
    use super::*;
    use crate::guest::*;
    use crate::wasm_rt::buffer::*;

    use once_cell::sync::{Lazy};

    use std::collections::{HashMap};
    use std::sync::*;

    /// Guest runtimes using the Postcard encoding
    static GUEST_POSTCARD_RUNTIMES: Lazy<Mutex<HashMap<GuestRuntimeHandle, Arc<GuestRuntime<GuestPostcardEncoder>>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

    ///
    /// Registers a guest runtime and returns the handle which can be passed on to the host side of things
    ///
    pub fn register_postcard_runtime(new_runtime: GuestRuntime<GuestPostcardEncoder>) -> GuestRuntimeHandle {
        // Assign a handle and store in the guest list
        let handle = allocate_handle();
        GUEST_POSTCARD_RUNTIMES.lock().unwrap().insert(handle, Arc::new(new_runtime));

        handle
    }

    ///
    /// Sends a message to a guest subprogram in a runtime
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_postcard_send_message(runtime: GuestRuntimeHandle, target: GuestSubProgramHandle, postcard_data: BufferHandle) {
        // Get the postcard runtime with this ID
        let runtime = GUEST_POSTCARD_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();

        // Retrieve the postcard data buffer from where it was being written by the host
        let postcard_data = claim_buffer(postcard_data);

        // Send the message to the runtime
        runtime.send_message(target, postcard_data);
    }

    ///
    /// Indicates to a guest subprogram that it is safe to send to a sink
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_postcard_sink_ready(runtime: GuestRuntimeHandle, sink: HostSinkHandle) {
        let runtime = GUEST_POSTCARD_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();
        runtime.sink_ready(sink);
    }

    ///
    /// Indicates to aguest subprogram that an error ocurred while connecting a sink
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_postcard_sink_connection_error(runtime: GuestRuntimeHandle, sink: HostSinkHandle, postcard_error: BufferHandle) {
        let postcard_error  = claim_buffer(postcard_error);
        let error           = postcard::from_bytes(&postcard_error).unwrap();

        let runtime = GUEST_POSTCARD_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();
        runtime.sink_connection_error(sink, error);
    }

    ///
    /// Indicates to a guest subprogram that an error ocurred while sending data to a sink
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_postcard_sink_send_error(runtime: GuestRuntimeHandle, sink: HostSinkHandle, postcard_error: BufferHandle) {
        let postcard_error  = claim_buffer(postcard_error);
        let error           = postcard::from_bytes(&postcard_error).unwrap();

        let runtime = GUEST_POSTCARD_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();
        runtime.sink_send_error(sink, error);
    }
}

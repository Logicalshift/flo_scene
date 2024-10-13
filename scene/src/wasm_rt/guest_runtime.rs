use crate::guest::*;
use crate::wasm_rt::buffer::*;

use once_cell::sync::{Lazy};

use std::collections::{HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::*;

/// Guest runtimes using the JSON encoding
static GUEST_JSON_RUNTIMES: Lazy<Mutex<HashMap<GuestRuntimeHandle, Arc<GuestRuntime<GuestJsonEncoder>>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
fn allocate_handle() -> GuestRuntimeHandle {
    // The next handle to assign
    static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(0);

    let this_handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    GuestRuntimeHandle(this_handle)
}

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
pub extern "C" fn scene_guest_send_json_message(runtime: GuestRuntimeHandle, target: GuestSubProgramHandle, json_data: BufferHandle) {
    // Get the JSON runtime with this ID
    let runtime = GUEST_JSON_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();

    // Retrieve the JSON data buffer from where it was being written by the host
    let json_data = claim_buffer(json_data);

    // Send the message to the runtime
    runtime.send_message(target, json_data);
}

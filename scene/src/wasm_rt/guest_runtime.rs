use crate::guest::*;
use crate::host::serialization_context::*;

use std::sync::atomic::{AtomicUsize, Ordering};

///
/// Assigns a new guest runtime handle
///
pub (super) fn allocate_handle() -> GuestRuntimeHandle {
    // The next handle to assign
    static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(0);

    let this_handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    GuestRuntimeHandle(this_handle)
}

pub use postcard_runtime::*;

mod postcard_runtime {
    use super::*;
    use crate::wasm_rt::buffer::*;

    use once_cell::sync::{Lazy};

    use postcard;

    use std::collections::{HashMap};
    use std::sync::*;

    /// Guest runtimes using the Postcard encoding
    static GUEST_POSTCARD_RUNTIMES: Lazy<Mutex<HashMap<GuestRuntimeHandle, Arc<GuestRuntime>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

    ///
    /// Registers a guest runtime and returns the handle which can be passed on to the host side of things
    ///
    pub fn register_postcard_runtime(new_runtime: GuestRuntime) -> GuestRuntimeHandle {
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

    ///
    /// Performs the poll_awake runtime operation, filling a buffer, and returning it to the host. The return format is always using the 'postcard' serialziation
    /// here.
    ///
    /// The host should call scene_free_buffer on this buffer
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_postcard_poll_awake(runtime: GuestRuntimeHandle) -> BufferHandle {
        let runtime     = GUEST_POSTCARD_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();
        let result      = runtime.poll_awake();
        let serialized  = postcard::to_stdvec(&result).unwrap();

        buffer_store(serialized)
    }

    ///
    /// Sends a message to a stream on a guest
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_postcard_send_stream(runtime: GuestRuntimeHandle, stream_id: i32, message: BufferHandle) {
        let runtime = GUEST_POSTCARD_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();
        let message = claim_buffer(message);

        let stream_id = if stream_id >= 0 { SerializationId::SimpleStream(stream_id as _) } else { SerializationId::SimpleFunction(-(stream_id+1) as _) };

        runtime.send_stream(stream_id, message);
    }

    ///
    /// Indicates that a host stream is ready to receive another message
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_postcard_ready_stream(runtime: GuestRuntimeHandle, stream_id: i32) {
        let runtime = GUEST_POSTCARD_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();

        let stream_id = if stream_id >= 0 { SerializationId::SimpleStream(stream_id as _) } else { SerializationId::SimpleFunction(-(stream_id+1) as _) };

        runtime.ready_stream(stream_id);
    }

    ///
    /// Indicates that a stream has been closed on the host side
    ///
    #[no_mangle]
    pub extern "C" fn scene_guest_postcard_close_stream(runtime: GuestRuntimeHandle, stream_id: i32) {
        let runtime = GUEST_POSTCARD_RUNTIMES.lock().unwrap().get(&runtime).unwrap().clone();

        let stream_id = if stream_id >= 0 { SerializationId::SimpleStream(stream_id as _) } else { SerializationId::SimpleFunction(-(stream_id+1) as _) };

        runtime.close_stream(stream_id);
    }
}

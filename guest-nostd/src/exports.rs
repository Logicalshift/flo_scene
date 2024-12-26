//!
//! The functions that are exported to the host in order to allow it to communicate with the guest
//!

use crate::guest_types::*;
use crate::runtime::*;

use once_cell::race::{OnceBox};
use core::cell::{UnsafeCell};
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::boxed::*;
use alloc::sync::*;
use alloc::vec::*;

/// Guest runtimes using the Postcard encoding
static GUEST_RUNTIMES: OnceBox<Shared<Vec<Option<Arc<GuestRuntime>>>>>      = OnceBox::new();

static BUFFERS:         OnceBox<Shared<Vec<Option<UnsafeCell<Vec<u8>>>>>>   = OnceBox::new();
static FREE_BUFFERS:    OnceBox<Shared<Vec<BufferHandle>>>                  = OnceBox::new();
static NEXT_BUFFER:     AtomicUsize                                         = AtomicUsize::new(0);

#[inline]
fn guest_runtimes<'a>() -> &'a Shared<Vec<Option<Arc<GuestRuntime>>>> {
    &*GUEST_RUNTIMES.get_or_init(|| Box::new(share(Vec::new())))
}

#[inline]
fn buffers<'a>() -> &'a Shared<Vec<Option<UnsafeCell<Vec<u8>>>>> {
    &*BUFFERS.get_or_init(|| Box::new(share(Vec::new())))
}

#[inline]
fn free_buffers<'a>() -> &'a Shared<Vec<BufferHandle>> {
    &*FREE_BUFFERS.get_or_init(|| Box::new(share(Vec::new())))
}

///
/// Handle to a buffer in a scene (these are used for transferring data to and from a webassembly module)
///
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
#[repr(transparent)]
pub struct BufferHandle(usize);

impl BufferHandle {
    ///
    /// Allocates a new buffer
    ///
    #[inline]
    pub fn new() -> Self {
        BufferHandle(NEXT_BUFFER.fetch_add(1, Ordering::Relaxed))
    }
}

///
/// Creates a new buffer on the guest side (this should be used so that no buffers can clash)
///
#[no_mangle]
pub unsafe extern "C" fn scene_new_buffer() -> BufferHandle {
    if let Some(reused_handle) = with_shared(free_buffers(), |free_buffers| free_buffers.pop()) {
        reused_handle
    } else {
        BufferHandle::new()
    }
}

///
/// Borrows a buffer until scene_return_buffer is called to return the buffer to the store
///
/// Used for allocating or retrieving space to use to load data from the host runtime. The caller is expected
/// to manually manage the lifetime of the returned buffer (must not use the reference again after re-entering the
/// webassembly module)
///
#[no_mangle]
pub unsafe extern "C" fn scene_borrow_buffer(buffer_handle: BufferHandle, buffer_size: usize) -> *mut u8 {
    // Retrieve the buffer (assuming nothing else is using it!)
    with_shared(buffers(), |buffers| {
        while buffers.len() <= buffer_handle.0 { buffers.push(None); }

        let buffer      = buffers[buffer_handle.0].get_or_insert_with(|| UnsafeCell::new(vec![0; buffer_size]));
        let contents    = buffer.get();

        // Resize it if needed
        if (*contents).len() != buffer_size {
            (*contents).resize(buffer_size, 0);
        }

        // Return the buffer to the caller
        (*contents).as_mut_ptr()
    })
}

///
/// Releases a buffer from the host side, freeing the memory it's using and allowing the handle to be re-used
///
#[no_mangle]
pub unsafe extern "C" fn scene_buffer_size(buffer_handle: BufferHandle) -> usize {
    with_shared(buffers(), |buffers| {
        if let Some(Some(buffer)) = buffers.get(buffer_handle.0) {
            unsafe { (*buffer.get()).len() }
        } else {
            0
        }
    })
}

///
/// Releases a buffer from the host side, freeing the memory it's using and allowing the handle to be re-used
///
#[no_mangle]
pub unsafe extern "C" fn scene_free_buffer(buffer_handle: BufferHandle) {
    with_shared(buffers(), |buffers| {
        if let Some(_) = buffers[buffer_handle.0].take() {
            // Add to the set of free buffers so we'll re-use this handle
            with_shared(free_buffers(), |free_buffers| free_buffers.push(buffer_handle));
        }
    })
}

///
/// Claims a buffer from the native side
///
pub fn claim_buffer(buffer_handle: BufferHandle) -> Vec<u8> {
    with_shared(buffers(), |buffers| {
        // Remove the buffer from the BTreeMap and return it after unwrapping it from its cell
        if let Some(buffer) = buffers[buffer_handle.0].take() {
            // Add to the set of free buffers so we'll re-use this handle
            with_shared(free_buffers(), |free_buffers| free_buffers.push(buffer_handle));

            buffer.into_inner()
        } else {
            Vec::new()
        }
    })
}

///
/// Stores a Vec<u8> as a buffer and returns the handle
///
pub (super) fn buffer_store(data: Vec<u8>) -> BufferHandle {
    let handle = BufferHandle::new();
    with_shared(buffers(), move |buffers| {
        let handle = handle.0;
        while buffers.len() <= handle {
            buffers.push(None);
        }

        buffers[handle] = Some(UnsafeCell::new(data));
    });
    handle
}

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
pub fn register_postcard_runtime(new_runtime: GuestRuntime) -> GuestRuntimeHandle {
    // Assign a handle and store in the guest list
    let handle = allocate_handle();
    with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes[handle.0] = Some(Arc::new(new_runtime)));

    handle
}

///
/// Sends a message to a guest subprogram in a runtime
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_send_message(runtime: GuestRuntimeHandle, target: GuestSubProgramHandle, postcard_data: BufferHandle) {
    // Get the postcard runtime with this ID
    let runtime = with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes.get(runtime.0).cloned());

    if let Some(Some(runtime)) = runtime {
        // Retrieve the postcard data buffer from where it was being written by the host
        let postcard_data = claim_buffer(postcard_data);

        // Send the message to the runtime
        runtime.send_message(target, postcard_data);
    }
}

///
/// Indicates to a guest subprogram that it is safe to send to a sink
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_sink_ready(runtime: GuestRuntimeHandle, sink: HostSinkHandle) {
    let runtime = with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes.get(runtime.0).cloned());

    if let Some(Some(runtime)) = runtime {
        runtime.sink_ready(sink);
    }
}

///
/// Indicates to aguest subprogram that an error ocurred while connecting a sink
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_sink_connection_error(runtime: GuestRuntimeHandle, sink: HostSinkHandle, postcard_error: BufferHandle) {
    let postcard_error  = claim_buffer(postcard_error);
    let error           = postcard::from_bytes(&postcard_error).unwrap();

    let runtime = with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes.get(runtime.0).cloned());
    if let Some(Some(runtime)) = runtime {
        runtime.sink_connection_error(sink, error);
    }
}

///
/// Indicates to a guest subprogram that an error ocurred while sending data to a sink
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_sink_send_error(runtime: GuestRuntimeHandle, sink: HostSinkHandle, postcard_error: BufferHandle) {
    let postcard_error  = claim_buffer(postcard_error);
    let error           = postcard::from_bytes(&postcard_error).unwrap();

    let runtime = with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes.get(runtime.0).cloned());
    if let Some(Some(runtime)) = runtime {
        runtime.sink_send_error(sink, error);
    }
}

///
/// Performs the poll_awake runtime operation, filling a buffer, and returning it to the host. The return format is always using the 'postcard' serialziation
/// here.
///
/// The host should call scene_free_buffer on this buffer
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_poll_awake(runtime: GuestRuntimeHandle) -> BufferHandle {
    let runtime = with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes.get(runtime.0).cloned());

    let result = if let Some(Some(runtime)) = runtime {
        runtime.poll_awake()
    } else {
        vec![]
    };

    let serialized  = postcard::to_stdvec(&result).unwrap();
    buffer_store(serialized)
}

///
/// Sends a message to a stream on a guest
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_send_stream(runtime: GuestRuntimeHandle, stream_id: i32, message: BufferHandle) {
    let runtime = with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes.get(runtime.0).cloned());
    let message = claim_buffer(message);

    let stream_id = if stream_id >= 0 { SerializationId::MyStream(stream_id as _) } else { SerializationId::TheirStream(-(stream_id+1) as _) };

    if let Some(Some(runtime)) = runtime {
        runtime.send_stream(stream_id, message);
    }
}

///
/// Indicates that a host stream is ready to receive another message
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_ready_stream(runtime: GuestRuntimeHandle, stream_id: i32) {
    let runtime = with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes.get(runtime.0).cloned());

    let stream_id = if stream_id >= 0 { SerializationId::MyStream(stream_id as _) } else { SerializationId::TheirStream(-(stream_id+1) as _) };

    if let Some(Some(runtime)) = runtime {
        runtime.ready_stream(stream_id);
    }
}

///
/// Indicates that a stream has been closed on the host side
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_close_stream(runtime: GuestRuntimeHandle, stream_id: i32) {
    let runtime = with_shared(guest_runtimes(), |guest_runtimes| guest_runtimes.get(runtime.0).cloned());

    let stream_id = if stream_id >= 0 { SerializationId::MyStream(stream_id as _) } else { SerializationId::TheirStream(-(stream_id+1) as _) };

    if let Some(Some(runtime)) = runtime {
        runtime.close_stream(stream_id);
    }
}

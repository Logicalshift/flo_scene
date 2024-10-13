use once_cell::sync::{Lazy};

use std::cell::{UnsafeCell};
use std::collections::{HashMap};
use std::sync::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static BUFFERS: Lazy<Mutex<HashMap<BufferHandle, UnsafeCell<Vec<u8>>>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_BUFFER: AtomicUsize = AtomicUsize::new(0);

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
    BufferHandle::new()
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
    let mut buffers = BUFFERS.lock().unwrap();
    let buffer      = buffers.entry(buffer_handle).or_insert_with(|| UnsafeCell::new(vec![0; buffer_size]));
    let contents    = buffer.get();

    // Resize it if needed
    if (*contents).len() != buffer_size {
        (*contents).resize(buffer_size, 0);
    }

    // Return the buffer to the caller
    (*contents).as_mut_ptr()
}

///
/// Claims a buffer from the native side
///
pub fn claim_buffer(buffer_handle: BufferHandle) -> Vec<u8> {
    let mut buffers = BUFFERS.lock().unwrap();

    // Remove the buffer from the hashmap and return it after unwrapping it from its cell
    if let Some(buffer) = buffers.remove(&buffer_handle) {
        buffer.into_inner()
    } else {
        vec![]
    }
}

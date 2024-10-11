use once_cell::sync::{Lazy};

use std::cell::{UnsafeCell};
use std::collections::{HashMap};
use std::sync::*;

static BUFFERS: Lazy<Mutex<HashMap<usize, UnsafeCell<Vec<u8>>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

///
/// Borrows a buffer until scene_return_buffer is called to return the buffer to the store
///
/// Used for allocating or retrieving space to use to load data from the host runtime. The caller is expected
/// to manually manage the lifetime of the returned buffer (must not use the reference again after re-entering the
/// webassembly module)
///
#[no_mangle]
pub unsafe extern "C" fn scene_borrow_buffer(buffer_handle: usize, buffer_size: usize) -> *mut u8 {
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

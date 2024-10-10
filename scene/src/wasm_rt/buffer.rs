use once_cell::sync::{Lazy};


///
/// Retrieves a byte buffer of the specified size (we trust the caller to not call us back with the buffer still borrowed)
///
#[no_mangle]
pub extern "C" fn scene_buffer(handle: usize, buffer_size: usize) /* -> &'a mut [u8] */ {

}

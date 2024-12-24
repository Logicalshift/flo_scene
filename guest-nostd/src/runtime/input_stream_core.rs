use futures::task::{Waker};

use alloc::collections::{VecDeque};
use alloc::vec::*;

///
/// The input stream core is used in
///
pub (crate) struct GuestInputStreamCore {
    /// Messages waiting in this input stream
    waiting: VecDeque<Vec<u8>>,

    /// Waker for the future for this input stream
    waker: Option<Waker>,

    /// Set to true once the stream should be considered to be closed
    closed: bool,

    /// Set to true when the stream is ready (and false when input is returned)
    is_ready: bool,
}

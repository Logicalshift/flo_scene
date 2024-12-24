use crate::guest_types::*;

use futures::task::{Waker};

use alloc::collections::{VecDeque};
use alloc::vec::*;

///
/// The input stream core is used in
///
pub (crate) struct GuestInputStreamCore {
    /// Messages waiting in this input stream
    pub (super) waiting: VecDeque<Vec<u8>>,

    /// Waker for the future for this input stream
    pub (super) waker: Option<Waker>,

    /// Set to true once the stream should be considered to be closed
    pub (super) closed: bool,

    /// Set to true when the stream is ready (and false when input is returned)
    pub (super) is_ready: bool,
}

impl GuestInputStreamCore {
    ///
    /// Enqueues a message into an input stream core, returning the waker for the future
    ///
    pub (crate) fn send_message(core: &Shared<GuestInputStreamCore>, message: Vec<u8>) -> Option<Waker> {
        with_shared(core, |core| {
            // Enqueue the message
            core.waiting.push_back(message);

            // Return the waker if there is one
            core.waker.take()
        })
    }
}

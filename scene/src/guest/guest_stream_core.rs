use futures::task::{Waker};

///
/// Core of a guest stream
///
pub (crate) struct GuestStreamCore {
    /// The pending messages for this stream
    pending: Vec<Vec<u8>>,

    /// Waker to be signalled when new data is pending
    waker: Waker,
}

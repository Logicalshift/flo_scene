use futures::task::{Waker};

///
/// Core of a guest stream
///
pub (crate) struct GuestStreamCore {
    /// The pending messages for this stream
    pub (crate) pending: Vec<Vec<u8>>,

    /// Waker to be signalled when new data is pending
    pub (crate) waker: Option<Waker>,

    /// True if this stream has been closed by the host
    pub (crate) closed: bool,
}

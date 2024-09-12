use crate::host::error::*;

use futures::task::{Waker};

///
/// Handle that identifies an output sink on the host side
///
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostSinkHandle(pub usize);

///
/// The current state of a guest sink
///
pub (crate) enum GuestSinkStatus {
    /// Message has been sent or we're waiting for connection
    Busy,

    /// Ready to receive a message
    Ready,

    /// Sink failed to connect
    ConnectionError(ConnectionError),

    /// Sink received a send error
    SendError(SceneSendError<Vec<u8>>)
}

///
/// Data attached to a guest sink
///
pub (crate) struct GuestSink {
    /// Wakes this sink up when something happens
    pub (crate) waker: Option<Waker>,

    /// True when this sink is ready
    pub (crate) status: GuestSinkStatus,
}

use super::guest_types::*;
use super::host_types::*;

use serde::*;

use alloc::vec::*;

///
/// Results from a polling action (requests from the host side)
///
#[derive(Serialize, Deserialize)]
pub struct GuestPollResult {
    results: Vec<GuestResult>,
}

///
/// A result returned after a guest program has been polled
///
/// The guest should wait for the `Ready` message before trying to send any message, and also needs to wait again
/// after sending a message.
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GuestResult {
    /// Indicates that the guest has stopped running and won't accept any further requests
    Stopped,

    /// The guest has created a subprogram, which can be referred to using the specified subprogram handle.
    /// Externally (on the host), it should be referred to by the subprogram ID instead
    /// The host stream ID indicates the type of data that can be sent to the subprogram
    CreateSubprogram(SubProgramId, GuestSubProgramHandle, HostStreamId),

    /// The specified subprogram has ended and cannot accept any more messages
    EndedSubprogram(GuestSubProgramHandle),

    /// Indicates that the specified guest subprogram is ready to receive a message (when a program is created or when a message sent to it, it should be considered
    /// 'not ready' until this is received)
    Ready(GuestSubProgramHandle),

    /// Creates a connection to a target on the host side
    Connect(HostSinkHandle, HostStreamTarget),

    /// Sends data to a sink established on the target side (which must have indicated that it's 'ready')
    Send(HostSinkHandle, Vec<u8>),

    /// Remove the connection associated with a sink handle
    Disconnect(HostSinkHandle),

    /// The guest still has more work to do and should be immediately polled again
    ContinuePolling,

    /// Sends a message to a stream on the host
    SendStream(SerializationId, Vec<u8>),

    /// Indicates that a stream is ready to accept more messages from the host
    ReadyStream(SerializationId),

    /// Indicates that a stream has been closed on the host side
    CloseStream(SerializationId),
}

impl GuestPollResult {
    ///
    /// Return no results
    ///
    pub fn empty() -> Self {
        GuestPollResult {
            results: Vec::new()
        }
    }

    ///
    /// Return the specified result only
    ///
    pub fn with_action(result: GuestResult) -> Self {
        GuestPollResult { 
            results: [result].into()
        }
    }

    ///
    /// Return multiple results
    ///
    pub fn with_actions(results: impl IntoIterator<Item=GuestResult>) -> Self {
        GuestPollResult { 
            results: results.into_iter().collect()
        }
    }
}

impl IntoIterator for GuestPollResult {
    type Item       = GuestResult;
    type IntoIter   = <Vec<GuestResult> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.results.into_iter()
    }
}

use super::sink_handle::*;
use super::stream_target::*;
use super::subprogram_handle::*;
use crate::subprogram_id::*;

///
/// Results from a polling action (requests from the host side)
///
pub struct GuestPollResult {
    results: Vec<GuestResult>,
}

///
/// A result returned after a guest program has been polled
///
/// The guest should wait for the `Ready` message before trying to send any message, and also needs to wait again
/// after sending a message.
///
#[derive(Clone, Debug)]
pub enum GuestResult {
    /// Indicates that the guest has stopped running and won't accept any further requests
    Stopped,

    /// Indicates that the guest has internally created a new subprogram that should appear in the scene with the specified subprogram ID
    /// `AssignSubProgram` should be used to give this subprogram a handle for message processing purposes
    CreateSubprogram(SubProgramId),

    /// The specified subprogram has ended and cannot accept any more messages
    EndedSubprogram(GuestSubProgramHandle),

    /// Indicates that the specified guest subprogram is ready to receive a message
    Ready(GuestSubProgramHandle),

    /// Creates a connection to a target on the host side
    Connect(HostSinkHandle, HostStreamTarget),

    /// Sends data to a sink established on the target side (which must have indicated that it's 'ready')
    Send(HostSinkHandle, Vec<u8>),

    /// Remove the connection associated with a sink handle
    Disconnect(HostSinkHandle),
}

impl GuestPollResult {
    ///
    /// Return no results
    ///
    pub fn empty() -> Self {
        GuestPollResult {
            results: vec![]
        }
    }

    ///
    /// Return the specified result only
    ///
    pub fn with_action(result: GuestResult) -> Self {
        GuestPollResult { 
            results: vec![result]
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

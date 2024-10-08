use super::subprogram_handle::*;
use super::sink_handle::*;
use crate::host::error::*;

///
/// Action requests sent from a host to a guest
///
pub struct GuestPollAction {
    /// The actions that need to be carried out as part of this poll request
    actions: Vec<GuestAction>,
}

///
/// A guest action request
///
/// The host must not generate `SendMessage` until a `GuestResult::Ready` message has been received from the guest, and must not generate
/// `SendMessage` again until another `GuestResult::Ready` message is sent.
///
/// Similarly, the host will indicate that the guest can send messages to a sink with the `GuestAction::Ready` message, and the
/// host will expect the guest to not send more messages for a specific sink until the host indicates that it's ready.
///
#[derive(Clone, Debug, PartialEq)]
pub enum GuestAction {
    /// Sends a message encoded as bytes to a subprogram identified by ID
    SendMessage(GuestSubProgramHandle, Vec<u8>),

    /// The specified host sink is ready to accept a message
    Ready(HostSinkHandle),

    /// A sink creation request failed with an error
    SinkConnectionError(HostSinkHandle, ConnectionError),

    /// A message could not be sent to a sink
    SinkError(HostSinkHandle, SceneSendError<Vec<u8>>)
}

impl GuestPollAction {
    ///
    /// Poll the guest with no specific action to perform
    ///
    pub fn empty() -> Self {
        GuestPollAction {
            actions: vec![]
        }
    }

    ///
    /// Poll the guest with a single action
    ///
    pub fn with_action(action: GuestAction) -> Self {
        GuestPollAction { 
            actions: vec![action]
        }
    }

    ///
    /// Poll the guest with many actions
    ///
    pub fn with_actions(actions: impl IntoIterator<Item=GuestAction>) -> Self {
        GuestPollAction { 
            actions: actions.into_iter().collect()
        }
    }
}

impl IntoIterator for GuestPollAction {
    type Item = GuestAction;

    type IntoIter = <Vec<GuestAction> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.actions.into_iter()
    }
}

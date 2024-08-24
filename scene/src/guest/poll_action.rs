use super::subprogram_handle::*;
use crate::subprogram_id::*;

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
#[derive(Clone, Debug)]
pub enum GuestAction {
    /// Request to start the guest subprogram (each guest has only one 'core' subprogram). Value indicates the ID assigned to this instance of the
    /// subprogram.
    StartSubProgram(GuestSubProgramHandle),

    /// In response to a request indicating that a guest has a new subprogram, assigns a new handle to it for the purposes of sending/receiving messages
    AssignSubProgram(SubProgramId, GuestSubProgramHandle),

    /// Sends a message encoded as bytes to a subprogram identified by ID
    SendMessage(usize, Vec<u8>),
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

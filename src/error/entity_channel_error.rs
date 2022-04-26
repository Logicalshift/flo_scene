use futures::channel::mpsc;
use futures::channel::oneshot;

///
/// Errors that can occur when sending to an entity channel
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EntityChannelError {
    /// The entity is no longer listening for these kinds of message
    NotListening,
}

impl From<oneshot::Canceled> for EntityChannelError {
    fn from(_: oneshot::Canceled) -> EntityChannelError {
        EntityChannelError::NotListening
    }
}

impl From<mpsc::SendError> for EntityChannelError {
    fn from(_: mpsc::SendError) -> EntityChannelError {
        EntityChannelError::NotListening
    }
}

use super::scene_context_error::*;

use futures::channel::mpsc;
use futures::channel::oneshot;

///
/// Errors that can occur when sending to an entity channel
///
#[derive(Clone, PartialEq, Debug)]
pub enum EntityChannelError {
    /// The requested entity doesn't exist
    NoSuchEntity,

    /// The entity is no longer listening for these kinds of message
    NoLongerListening,

    /// A dynamic channel was expecting an entity of a particular type. The parameter is the name of the expected type.
    WrongMessageType(String),

    /// A dynamic message has already been processed
    MissingMessage,

    /// The message type was invalid for a particular entity
    WrongChannelType(String),

    /// No scene is available to create the channel
    NoCurrentScene,

    /// The scene context is not available because the scene has finished
    SceneFinished,

    /// The scene was requested from a point where the context was no longer available
    ThreadShuttingDown,

    /// The specified property is not defined
    NoSuchProperty,
}

impl From<oneshot::Canceled> for EntityChannelError {
    fn from(_: oneshot::Canceled) -> EntityChannelError {
        EntityChannelError::NoLongerListening
    }
}

impl From<mpsc::SendError> for EntityChannelError {
    fn from(_: mpsc::SendError) -> EntityChannelError {
        EntityChannelError::NoLongerListening
    }
}

impl From<SceneContextError> for EntityChannelError {
    fn from(error: SceneContextError) -> EntityChannelError {
        EntityChannelError::from(&error)
    }
}

impl From<&SceneContextError> for EntityChannelError {
    fn from(error: &SceneContextError) -> EntityChannelError {
        match error {
            SceneContextError::NoCurrentScene       => EntityChannelError::NoCurrentScene,
            SceneContextError::SceneFinished        => EntityChannelError::SceneFinished,
            SceneContextError::ThreadShuttingDown   => EntityChannelError::ThreadShuttingDown,
        }
    }
}

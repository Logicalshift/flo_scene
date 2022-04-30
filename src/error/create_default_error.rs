use super::scene_context_error::*;

///
/// Errors that can occur while creating a default behaviour
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CreateDefaultError {
    /// Default behaviour is already defined for the requested message type
    AlreadyExists,

    /// Tried to create an entity without a current scene
    NoCurrentScene,

    /// The scene was requested from a point where the context was no longer available
    ThreadShuttingDown,
}

impl From<SceneContextError> for CreateDefaultError {
    fn from(error: SceneContextError) -> CreateDefaultError {
        CreateDefaultError::from(&error)
    }
}

impl From<&SceneContextError> for CreateDefaultError {
    fn from(error: &SceneContextError) -> CreateDefaultError {
        match error {
            SceneContextError::NoCurrentScene       => CreateDefaultError::NoCurrentScene,
            SceneContextError::ThreadShuttingDown   => CreateDefaultError::ThreadShuttingDown,
        }
    }
}

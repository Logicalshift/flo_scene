use super::scene_context_error::*;

///
/// Errors that can occur while creating a new entity
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CreateEntityError {
    /// The entity that is being created already exists
    AlreadyExists,

    /// Tried to create an entity without a current scene
    NoCurrentScene,

    /// The scene was requested from a point where the context was no longer available
    ThreadShuttingDown,
}

impl From<SceneContextError> for CreateEntityError {
    fn from(error: SceneContextError) -> CreateEntityError {
        CreateEntityError::from(&error)
    }
}

impl From<&SceneContextError> for CreateEntityError {
    fn from(error: &SceneContextError) -> CreateEntityError {
        match error {
            SceneContextError::NoCurrentScene       => CreateEntityError::NoCurrentScene,
            SceneContextError::ThreadShuttingDown   => CreateEntityError::ThreadShuttingDown,
        }
    }
}

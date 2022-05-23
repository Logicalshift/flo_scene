use super::scene_context_error::*;

///
/// Errors relating to managing background futures for an entity
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EntityFutureError {
    /// Can't create a backgroudn future for an entity that's not running/doesn't exist
    NoSuchEntity,

    /// The program is not executing in a context for a particular entity
    NoCurrentEntity,

    /// The program is not executing in a context where a scene is available
    NoCurrentScene,

    /// The scene context is not available because the scene has finished
    SceneFinished,

    /// The scene was requested from a point where the context was no longer available
    ThreadShuttingDown,
}

impl From<SceneContextError> for EntityFutureError {
    fn from(error: SceneContextError) -> EntityFutureError {
        EntityFutureError::from(&error)
    }
}

impl From<&SceneContextError> for EntityFutureError {
    fn from(error: &SceneContextError) -> EntityFutureError {
        match error {
            SceneContextError::NoCurrentScene       => EntityFutureError::NoCurrentScene,
            SceneContextError::SceneFinished        => EntityFutureError::SceneFinished,
            SceneContextError::ThreadShuttingDown   => EntityFutureError::ThreadShuttingDown,
        }
    }
}

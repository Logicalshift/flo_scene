///
/// Requests for controlling a scene as a whole
///
pub enum SceneControlRequest {
    /// Requests that the main scene runtime stop (which will stop all the entities in the scene)
    StopScene,
}

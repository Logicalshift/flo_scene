//!
//! A 'main' scene is used to coordinate communications between all other scenes in an application
//! as well as with external entities. Support for external communication via a UNIX pipe is supported
//! by default.
//!

use flo_scene::*;

/// The subprogram ID used to communicate with the main scene from a sub-scene
// pub static MAIN_SCENE_ID: SubProgramId = SubProgramId::called("Main scene");

///
/// Requests that can be made to a main scene from another scene
///
#[derive(Clone)]
pub enum MainScene {
    /// Specify a friendly name for this scene
    FriendlyName(String),

    /// Allows access to a stream via the SubScene interface in the main scene
    Publish(StreamId),
}

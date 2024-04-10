//!
//! A 'main' scene is used to coordinate communications between all other scenes in an application
//! as well as with external entities. Support for external communication via a UNIX pipe is supported
//! by default.
//!

use flo_scene::*;

use once_cell::sync::{Lazy};

use std::sync::*;

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

///
/// Creates the main scene object
///
fn create_main_scene() -> Scene {
    let main_scene = Scene::default();

    main_scene
}

///
/// Retrieves or creates the main scene
///
pub fn main_scene() -> Scene {
    static MAIN_SCENE: Lazy<Mutex<Scene>> = Lazy::new(|| Mutex::new(create_main_scene()));

    let main_scene = MAIN_SCENE.lock().unwrap();

    (*main_scene).clone()
}
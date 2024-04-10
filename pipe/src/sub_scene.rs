//!
//! The 'sub-scene' subprogram can be used to send and receive messages from another scene
//!

use flo_scene::*;

use serde::*;
use uuid::{Uuid};

#[derive(Clone, Serialize, Deserialize)]
pub struct SubSceneId(Uuid);

///
/// Requests that can be made to a subscene from the main scene
///
pub enum SubScene {
    /// Send a list of the available sub-scenes to the specified subprogram ID
    List(SubProgramId),

    /// Creates a subprogram in this scene, with ID `our_program` to `their_progam` in the specified subscene
    Connect { scene: SubSceneId, their_program: SubProgramId, our_program: SubProgramId },

    /// Receives a stream directed at the 'main scene' program in a sub-scene into a program in the current scene
    Receive { scene: SubSceneId, stream: StreamId, target: SubProgramId },
}

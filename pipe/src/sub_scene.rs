//!
//! The 'sub-scene' subprogram can be used to send and receive messages from another scene
//!

use flo_scene::*;

use serde::*;
use uuid::{Uuid};

use std::any::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct SubSceneId(Uuid);

///
/// Requests that can be made to a subscene from the main scene
///
pub enum SubScene {
    /// Send a list of the available sub-scenes to the specified subprogram ID
    List(SubProgramId),

    /// Sends a message to a subscene
    Send(SubSceneId, Box<dyn Any>),
}

use crate::subprogram_id::*;
use crate::input_stream::*;
use crate::scene_context::*;

use futures::prelude::*;
use once_cell::sync::{Lazy};

/// The outside scene program is a program that relays messages generated from a stream outside of a scene: it doesn't do anything directly itself
pub static OUTSIDE_SCENE_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("OUTSIDE_SCENE_PROGRAM"));

///
/// Runs the outside scene program
///
pub (crate) async fn outside_scene_program(input: InputStream<()>, _context: SceneContext) {
    // All this program does is ignore its messages until it finishes
    let mut input = input;
    while let Some(_input) = input.next().await { }
}

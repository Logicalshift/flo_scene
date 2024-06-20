use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// The `list_subprograms` command, which lists the subprograms in the current scene
///
pub fn command_list_subprograms(input: serde_json::Value, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

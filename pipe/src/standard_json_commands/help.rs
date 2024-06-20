use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;

///
/// The 'help' command, which generates some help text
///
pub fn command_help(input: serde_json::Value, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

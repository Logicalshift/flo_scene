use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// The arguments to the connect command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct SendArguments {

}

///
/// The `send` command, which sends messags to a subprogram in a scene
///
pub fn command_send(input: SendArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

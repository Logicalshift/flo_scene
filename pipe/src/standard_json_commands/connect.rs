use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// The arguments to the connect command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct ConnectArguments {

}

///
/// The `connect` command, which connects two subprograms in a scene
///
pub fn command_connect(input: ConnectArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

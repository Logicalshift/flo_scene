use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// The arguments to the connect command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct SubscribeArguments {

}

///
/// The `subscribe` command, which opens a background stream to events from a source subprogram
///
pub fn command_subscribe(input: SubscribeArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

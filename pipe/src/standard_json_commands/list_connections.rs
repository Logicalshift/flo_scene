use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// The `list_connections` command, which lists the connections that are defined between subprograms
///
pub fn command_list_connections(input: serde_json::Value, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

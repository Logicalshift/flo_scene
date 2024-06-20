use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// The arguments to the query command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct QueryArguments {

}

///
/// The `query` command, which runs a query and returns the results
///
pub fn command_query(input: QueryArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

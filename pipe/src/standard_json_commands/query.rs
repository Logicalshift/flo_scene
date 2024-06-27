use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// The arguments to the query command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct QueryArguments {
    /// The serializable
    response_type_name: String,

    /// The target of the query
    query_target: Option<SubProgramId>,
}

///
/// The `query` command, which runs a query and returns the results
///
pub fn command_query(input: QueryArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

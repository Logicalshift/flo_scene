use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// A response to a list subprograms request
///
#[derive(Clone, Serialize, Deserialize)]
pub struct ListSubprogramsResponse {
    /// The ID of this subprogram
    id: SubProgramId,

    /// The type_name of the input stream for this subprogram
    rust_type_description: String,

    /// If the input stream can be serialized, this is the serialization name of the type (can be used with 'Send', say)
    serialized_type_name: Option<String>,
}

///
/// The `list_subprograms` command, which lists the subprograms in the current scene
///
pub fn command_list_subprograms(input: serde_json::Value, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        CommandResponse::Error("Not implemented".into())
    }
}

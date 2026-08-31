use crate::commands::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;

use std::borrow::*;
use std::collections::*;

///
/// A response to a list subprograms request
///
#[derive(Clone, Serialize, Deserialize)]
pub struct ListSubprogramsResponse {
    /// The ID of this subprogram
    pub id: SubProgramId,

    /// Name for this program
    pub name: Option<Cow<'static, str>>,

    /// The type_name of the input stream for this subprogram (as a Rust type: note that this can vary and is informational only)
    pub rust_type_name: String,

    /// If the input stream can be serialized, this is the serialization name of the type (can be used with 'Send', say)
    pub serialized_type_name: Option<String>,
}

impl SceneMessage for ListSubprogramsResponse {
    #[inline]
    fn message_type_name() -> String { "flo_scene_pipe::ListSubprogramsResponse".into() }
}

impl Default for ListSubprogramsResponse {
    fn default() -> Self {
        ListSubprogramsResponse {
            id:             SubProgramId::called("Unknown"),
            name:           None,
            rust_type_name: "Unknown".into(),
            serialized_type_name: None,
        }
    }
}

///
/// The `list_subprograms` command, which lists the subprograms in the current scene
///
pub fn command_list_subprograms(_input: serde_json::Value, context: SceneContext) -> impl Future<Output=CommandResponseData<Vec<ListSubprogramsResponse>>> {
    async move {
        // Query the scene control program for the list of subprograms
        match context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), *SCENE_CONTROL_PROGRAM) {
            Ok(updates) => {
                let mut updates     = updates;
                let mut subprograms = HashMap::new();

                // Read the responses from the updates
                while let Some(update) = updates.next().await {
                    match update {
                        SceneUpdate::Started(program_id, input_stream_id) => {
                            // Fill in the details about this program
                            let details = subprograms.entry(program_id).or_insert_with(|| ListSubprogramsResponse::default());

                            details.id                      = program_id;
                            details.rust_type_name          = input_stream_id.message_type_name();
                            details.serialized_type_name    = input_stream_id.serialization_type_name();
                        }

                        SceneUpdate::Tagged(program_id, SceneProgramTag::Name(name)) => {
                            // Add the name for this program
                            let details = subprograms.entry(program_id).or_insert_with(|| ListSubprogramsResponse::default());

                            details.id      = program_id;
                            details.name    = details.name.take().or(Some(name));
                        }

                        _ => { }
                    }
                }

                CommandResponseData::Data(subprograms.into_iter().map(|(_, response)| response).collect())
            }

            Err(error) => {
                // Could not get the list of updates from the scene
                CommandResponseData::Error(format!("Could not query scene: {:?}", error))
            }
        }
    }
}

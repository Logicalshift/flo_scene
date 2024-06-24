use crate::commands::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;

///
/// A response to a list subprograms request
///
#[derive(Clone, Serialize, Deserialize)]
pub struct ListSubprogramsResponse {
    /// The ID of this subprogram
    pub id: SubProgramId,

    /// The type_name of the input stream for this subprogram (as a Rust type: note that this can vary and is informational only)
    pub rust_type_name: String,

    /// If the input stream can be serialized, this is the serialization name of the type (can be used with 'Send', say)
    pub serialized_type_name: Option<String>,
}

impl SceneMessage for ListSubprogramsResponse { }

///
/// The `list_subprograms` command, which lists the subprograms in the current scene
///
pub fn command_list_subprograms(_input: serde_json::Value, context: SceneContext) -> impl Future<Output=CommandResponseData<Vec<ListSubprogramsResponse>>> {
    async move {
        // Query the scene control program for the list of subprograms
        match context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), *SCENE_CONTROL_PROGRAM) {
            Ok(updates) => {
                let mut updates     = updates;
                let mut responses   = vec![];

                // Read the responses from the updates
                while let Some(update) = updates.next().await {
                    // TODO: add the input type of this program, if available
                    match update {
                        SceneUpdate::Started(program_id, input_stream_id) => {
                            // Create a response for every program that's running
                            responses.push(ListSubprogramsResponse { 
                                id:                     program_id, 
                                rust_type_name:         input_stream_id.message_type_name(), 
                                serialized_type_name:   input_stream_id.serialization_type_name(), 
                            })
                        }

                        _ => { }
                    }
                }

                CommandResponseData::Data(responses)
            }

            Err(error) => {
                // Could not get the list of updates from the scene
                CommandResponseData::Error(format!("Could not query scene: {:?}", error))
            }
        }
    }
}

use crate::commands::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;

use std::collections::{HashMap};

///
/// A response from the list connections command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct ListConnectionsResponse {
    /// The target program for the message
    pub target: SubProgramId,

    /// The sources that are connected to this program
    pub sources: Vec<ListConnectionsSource>,
}

///
/// A response from the list connections command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct ListConnectionsSource {
    /// The source program for the message
    pub source: SubProgramId,

    /// The target specified in the stream ID, or None if this connection is from any program to any other program
    pub stream_target: Option<SubProgramId>,

    /// The rust type name of the data being sent over this connection
    pub rust_type_name: String,

    /// If the type name is serializable, this is the name used to refer to the rust type by the deserializer
    pub serialized_type_name: Option<String>,
}

impl SceneMessage for ListConnectionsResponse {
    #[inline]
    fn message_type_name() -> String { "flo_scene_pipe::ListConnectionsResponse".into() }
}

///
/// The `list_connections` command, which lists the connections that are active between subprograms
///
pub fn command_list_connections(_input: serde_json::Value, context: SceneContext) -> impl Future<Output=CommandResponseData<Vec<ListConnectionsResponse>>> {
    async move {
        // Query the scene control program for the list of subprograms
        match context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), *SCENE_CONTROL_PROGRAM) {
            Ok(updates) => {
                let mut updates     = updates;
                let mut responses   = HashMap::new();

                // Read the responses from the updates
                while let Some(update) = updates.next().await {
                    match update {
                        SceneUpdate::Connected(source_id, target_id, stream_id) => {
                            // Create a response for every program that's running
                            responses.entry(target_id)
                                .or_insert_with(|| ListConnectionsResponse { target: target_id, sources: vec![] })
                                .sources.push(ListConnectionsSource {
                                    source:                 source_id,
                                    stream_target:          stream_id.target_program(),
                                    rust_type_name:         stream_id.message_type_name(),
                                    serialized_type_name:   stream_id.serialization_type_name(),
                                });
                        }

                        _ => { }
                    }
                }

                let responses = responses.drain().map(|(_, value)| value).collect::<Vec<_>>();

                CommandResponseData::Data(responses)
            }

            Err(error) => {
                // Could not get the list of updates from the scene
                CommandResponseData::Error(format!("Could not query scene: {:?}", error))
            }
        }
    }
}

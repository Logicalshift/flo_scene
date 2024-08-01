use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use serde::*;

///
/// The arguments to the 'Send' command, which sends a stream of one or more messages to a target program
///
#[derive(Clone, Serialize, Deserialize)]
pub enum SendArguments {
    /// Send to a specific subprogram (using the subprogram's input type, which must support JSON deserialization)
    SubProgram(SubProgramId),

    /// Send messages of a specific type to the default target, if there is one
    Type(String),
}

///
/// The `send` command, which sends messags to a subprogram in a scene
///
pub fn command_send(destination: SendArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        let connection = match destination {
            SendArguments::SubProgram(subprogram_id) => {
                // Send to the subprogram using a serialized JSON stream
                todo!()
            },

            SendArguments::Type(type_name) => {
                if let Some(stream_id) = StreamId::with_serialization_type(type_name) {
                    // Send serialized to a generic stream
                    todo!()
                } else {
                    // Err(ConnectionError::TargetNotAvailable)
                    todo!()
                }
            }
        };

        CommandResponse::Error("Not implemented".into())
    }
}

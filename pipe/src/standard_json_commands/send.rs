use crate::commands::*;

use flo_scene::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::channel::oneshot;
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
                    //todo!()
                } else {
                    // Err(ConnectionError::TargetNotAvailable)
                    // todo!()
                }
            }
        };

        let (send_responses, recv_responses)    = mpsc::channel(16);
        let (send_input, recv_input)            = oneshot::channel();

        // Open an IO stream
        if context.send_message(CommandResponse::IoStream(Box::new(move |input_stream| {
                send_input.send(input_stream).ok();
                recv_responses.boxed()
            }))).await.is_err() {
            return CommandResponse::Error("Could not create ouput stream".into());
        }

        // Wait for the stream to open
        let input_stream = recv_input.await;
        let input_stream = if let Ok(input_stream) = input_stream { input_stream } else { return CommandResponse::Error("Input stream did not correct".into()); };

        // TODO: actually serialize the data to send: for the moment we just echo it back again
        let mut input_stream    = input_stream;
        let mut send_responses  = send_responses;
        
        while let Some(msg) = input_stream.next().await {
            send_responses.send(msg).await.ok();
        }

        // Finished
        CommandResponse::Message("OK".into())
    }
}

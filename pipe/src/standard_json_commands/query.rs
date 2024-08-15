use crate::commands::*;

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::channel::mpsc;
use serde::*;
use serde_json::*;

///
/// The arguments to the query command
///
#[derive(Clone, Serialize, Deserialize)]
pub enum QueryArguments {
    /// Send the query to the defualt responder for the specified type
    Type(String),

    /// Query a specific subprogram
    SubProgram {
        program:    SubProgramId, 
        type_name:  String
    },
}

///
/// The `query` command, which runs a query and returns the results
///
pub fn command_query(input: QueryArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        // Get the stream ID for the message and the query type
        let type_name = match &input {
            QueryArguments::Type(type_name)                 => type_name.clone(),
            QueryArguments::SubProgram { type_name, .. }    => type_name.clone(),
        };

        let message_stream = StreamId::with_serialization_type(type_name.clone());
        let request_stream = StreamId::with_serialization_type(format!("query::{}", type_name));

        let (message_stream, request_stream) = if let (Some(message_stream), Some(request_stream)) = (message_stream, request_stream) {
            (message_stream, request_stream)
        } else {
            return CommandResponse::Error(format!("Could not find a serializer for the message type `{}`", type_name));
        };

        // Create a oneshot channel to generate the result stream
        let (send_result_stream, recv_result_stream) = oneshot::channel();

        // Create a subprogram that will receive the query result mesasges and relay as part of the results
        let results_program = SubProgramId::new();

        context.send_message(SceneControl::start_program(results_program, move |input, _context| async move {
            // TODO: if we go idle before this starts, then the query has no response

            // Read a query response and send it to the parent command
            let mut input = input;
            if let Some(response) = input.next().await {
                let response: QueryResponse<SerializedMessage<serde_json::Value>> = response;

                send_result_stream.send(response).ok();
            }
        }, 20)).await.ok();

        // Wait for the message receiver to arrive
        let query_response = recv_result_stream.await;
        let query_response = if let Ok(query_response) = query_response {
            query_response
        } else {
            return CommandResponse::Error("Did not receive a query response".into())
        };

        // Request a query for the program we just created (serialized form of the Query message)
        let query_request = json!(vec![json![{ 
            "Program": results_program
        }], serde_json::Value::Null]);

        let query_target = match input {
            QueryArguments::Type(_)                     => SerializedStreamTarget::Stream(request_stream),
            QueryArguments::SubProgram { program, .. }  => SerializedStreamTarget::Stream(request_stream.for_target(program)),
        };

        let query_stream = context.send_serialized::<serde_json::Value>(query_target);
        let mut query_stream = match query_stream {
            Ok(query_stream)    => query_stream,
            Err(err)            => { return CommandResponse::Error(format!("Could not send query request: {:?}", err)); }
        };

        if let Err(err) = query_stream.send(query_request).await {
            return CommandResponse::Error(format!("Could not send query request: {:?}", err));
        }

        // Read the response into a vec
        let mut query_response  = query_response;
        let mut results         = vec![];
        while let Some(SerializedMessage(json, type_id)) = query_response.next().await {
            if type_id == message_stream.message_type() {
                results.push(json);
            } else {
                context.send_message(CommandResponse::Error("Received a message of an unexpected type".into())).await.ok();
                break;
            }
        }

        // Return the response as a JSON value
        CommandResponse::Json(serde_json::Value::Array(results))
    }
}

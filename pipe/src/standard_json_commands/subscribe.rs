use crate::commands::*;

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::channel::oneshot;
use serde::*;
use serde_json::json;

///
/// The arguments to the connect command
///
#[derive(Clone, Serialize, Deserialize)]
pub enum SubscribeArguments {
    /// Subscribe to messages of the specified type
    Type(String),

    /// Subscribe to messages of a particular type from a particular subprogram
    SubProgram {
        program:    SubProgramId, 
        type_name:  String
    },
}

///
/// The `subscribe` command, which opens a background stream to events from a source subprogram
///
pub fn command_subscribe(input: SubscribeArguments, context: SceneContext) -> impl Future<Output=CommandResponse> {
    async move {
        // Get the stream ID for the message and the query type
        let type_name = match &input {
            SubscribeArguments::Type(type_name)                 => type_name.clone(),
            SubscribeArguments::SubProgram { type_name, .. }    => type_name.clone(),
        };

        let message_stream = StreamId::with_serialization_type(type_name.clone());
        let request_stream = StreamId::with_serialization_type(format!("subscribe::{}", type_name));

        let (message_stream, request_stream) = if let (Some(message_stream), Some(request_stream)) = (message_stream, request_stream) {
            (message_stream, request_stream)
        } else {
            return CommandResponse::Error(format!("Could not find a serializer for the message type `{}`", type_name));
        };

        // Create a oneshot channel to generate the result stream
        let (send_result_stream, recv_result_stream) = oneshot::channel();

        // Create a subprogram that will receive the subscription mesasges and relay to the background stream
        let subscriber_id = SubProgramId::new();

        context.send_message(SceneControl::start_program(subscriber_id, move |input, _context| async move {
            // Create a channel to send and receive the messages from this subscription request
            let (send, receive) = mpsc::channel(20);

            // Send it back to the command that's waiting to generate the background stream
            send_result_stream.send(receive).ok();

            let mut input = input;
            let mut send: mpsc::Sender<serde_json::Value> = send;
            while let Some(SerializedMessage(json_message, type_id)) = input.next().await {
                if type_id == message_stream.message_type() {
                    if send.send(json_message).await.is_err() {
                        break;
                    }
                }
            }
        }, 20)).await.ok();

        // Wait for the message receiver to arrive
        let receiver = recv_result_stream.await;
        let receiver = if let Ok(receiver) = receiver {
            receiver
        } else {
            return CommandResponse::Error("Could not create message receiver".into())
        };

        // Request a subscription for the program we just created (serialized form of the Subscribe message)
        let subscription_request = json![{ 
            "Program": subscriber_id
        }];

        let subscribe_stream     = context.send_serialized::<serde_json::Value>(SerializedStreamTarget::Stream(request_stream));
        let mut subscribe_stream = match subscribe_stream {
            Ok(subscribe_stream)    => subscribe_stream,
            Err(err)                => { return CommandResponse::Error(format!("Could not send subscribe request: {:?}", err)); }
        };

        if let Err(err) = subscribe_stream.send(subscription_request).await {
            return CommandResponse::Error(format!("Could not send subscribe request: {:?}", err));
        }

        // Successful result is a background stream
        CommandResponse::BackgroundStream(receiver.boxed())
    }
}

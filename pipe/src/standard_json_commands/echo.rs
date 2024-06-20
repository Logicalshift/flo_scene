use crate::commands::*;

use flo_scene::*;
use futures::prelude::*;

use serde_json;

///
/// Function that implements the 'echo' command, which displays its input as a message
///
pub async fn command_echo(input: serde_json::Value, context: SceneContext) {
    // Get the command responses
    let responses       = context.send(());
    let mut responses   = if let Ok(responses) = responses { responses } else { return; };

    // Arrays have their values output as individual messages
    let messages = if let serde_json::Value::Array(messages) = input {
        messages
    } else {
        vec![input]
    };

    for input in messages {
        // Generate a message from the input
        match input {
            serde_json::Value::Null => {
                // Null values send an empty string
                responses.send(CommandResponse::Message("".to_string())).await.ok();
            }

            serde_json::Value::String(msg) => {
                // Strings are displayed directly as messages
                responses.send(CommandResponse::Message(msg)).await.ok();
            }

            serde_json::Value::Number(msg) => {
                // Numbers are also formatted directly
                responses.send(CommandResponse::Message(format!("{}", msg))).await.ok();
            }

            _ => {
                // Other JSON things are formatted as strings and then displayed
                let formatted = serde_json::to_string_pretty(&input);
                if let Ok(formatted) = formatted {
                    responses.send(CommandResponse::Message(formatted)).await.ok();
                }
            }
        }
    }
}

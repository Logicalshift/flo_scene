use crate::command_stream;
use crate::socket::*;

use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::commands::*;

use futures::prelude::*;
use futures::stream::{BoxStream};
use futures::channel::mpsc;

///
/// A connection to a simple command program
///
/// The simple command program can just read and write command responses, and cannot provide direct access to the terminal
///
pub type CommandProgramSocketMessage = SocketMessage<Result<command_stream::Command, ()>, command_stream::CommandResponse>;

///
/// The command program accepts connections from a socket and will generate command output messages
///
pub async fn command_connection_program(input: InputStream<CommandProgramSocketMessage>, context: SceneContext) {
    // Request that the socket send messages to this program
    // TODO: this would work a lot better if it was just a straight connection...
    let our_program_id = context.current_program_id().unwrap();
    context.send_message(Subscribe::<CommandProgramSocketMessage>::with_target(our_program_id.into())).await.unwrap();

    let spawn_connection = FnCommand::<Result<command_stream::Command, ()>, command_stream::CommandResponse>::new(command_connection);

    // Spawn processor tasks for each connection
    let mut input = input;
    while let Some(connection) = input.next().await {
        match connection {
            SocketMessage::Connection(connection) => {
                // Create a channel to receive the responses on
                let (send_response, recv_response) = mpsc::channel(0);
                let command_input = connection.connect(recv_response);

                // Spawn a reader for the command input
                if let Ok(responses) = context.spawn_command(spawn_connection.clone(), command_input) {
                    // ... and another task to send the responses from the command
                    context.spawn_command(FnCommand::<_, ()>::new(move |responses, _context| { 
                        let mut send_response = send_response.clone(); 
                        async move {
                            let mut responses = responses;
                            while let Some(response) = responses.next().await {
                                if send_response.send(response).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }), responses).ok();
                }
            }
        }
    }
}

///
/// Runs a command connection
///
async fn command_connection(input: BoxStream<'_, Result<command_stream::Command, ()>>, context: SceneContext) {
    let mut input       = input;
    let mut responses   = context.send::<command_stream::CommandResponse>(()).unwrap();

    while let Some(next_command) = input.next().await {
        match next_command {
            _ => {
                // Just send error responses
                if  responses.send(command_stream::CommandResponse::Error(format!("Not implemented yet"))).await.is_err() {
                    break;
                }
            }
        }
    }
}
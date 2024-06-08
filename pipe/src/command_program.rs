use crate::command_stream::*;
use crate::socket::*;

use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::commands::*;

use futures::{pin_mut};
use futures::prelude::*;
use futures::stream::{BoxStream};
use futures::channel::mpsc;

///
/// A connection to a simple command program
///
/// The simple command program can just read and write command responses, and cannot provide direct access to the terminal
///
pub type CommandProgramSocketMessage = SocketMessage<Result<CommandRequest, ()>, CommandResponse>;

///
/// The command program accepts connections from a socket and will generate command output messages
///
pub async fn command_connection_program(input: InputStream<CommandProgramSocketMessage>, context: SceneContext) {
    // Spawn processor tasks for each connection
    let mut input = input;
    while let Some(connection) = input.next().await {
        match connection {
            SocketMessage::Connection(connection) => {
                // Create a channel to receive the responses on
                // TODO: ideally we'd send the result of the 'spawn_command' routine to the connection here instead of relaying via another command
                // (but that requires a two-stage connection)
                let (send_response, recv_response) = mpsc::channel(0);
                let command_input = connection.connect(recv_response);

                // Spawn a reader for the command input
                if let Ok(responses) = context.spawn_command(CommandProcessor, command_input) {
                    // ... and another task to relay the responses back to the socket
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
/// The command processor command, which takes an input of parsed commands, and generates the corresponding responses
///
/// This will generate one response per command
///
#[derive(Copy, Clone, PartialEq)]
pub struct CommandProcessor;

impl Command for CommandProcessor {
    type Input  = Result<CommandRequest, ()>;
    type Output = CommandResponse;

    fn run(&self, input: impl 'static + Send + Stream<Item=Self::Input>, context: SceneContext) -> impl 'static + Send + Future<Output=()> {
        async move {
            pin_mut!(input);
            let mut responses   = context.send::<CommandResponse>(()).unwrap();

            while let Some(next_command) = input.next().await {
                use CommandRequest::*;

                let response = match next_command {
                    Ok(Command     { command, argument }) => { CommandResponse::Error("Not implemented yet".into()) }
                    Ok(Pipe        { from, to })          => { CommandResponse::Error("Not implemented yet".into()) }
                    Ok(Assign      { variable, from })    => { CommandResponse::Error("Not implemented yet".into()) }
                    Ok(ForTarget   { target, request })   => { CommandResponse::Error("Not implemented yet".into()) }
                    Err(_)                                => { CommandResponse::Error("Not implemented yet".into()) }
                };

                responses.send(response).await;
            }
        }
    }
}

use super::command_stream::*;
use super::json_command::*;
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
/// Commands will be sent to the command target (as `JsonCommand` requests). JsonCommand will create a default
/// dispatcher, which will send commands to whichever subprogram can respond: use `StreamTarget::Any` to target
/// this dispatcher.
///
/// (JsonCommands are a bit inefficient due to the need for a filter, but sending them will ensure that the dispatcher
/// is started)
///
pub async fn command_connection_program(input: InputStream<CommandProgramSocketMessage>, context: SceneContext, command_target: impl Into<StreamTarget>) {
    let command_target = command_target.into();

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
                if let Ok(responses) = context.spawn_command(CommandProcessor::new(command_target.clone()), command_input) {
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
#[derive(Clone, PartialEq)]
pub struct CommandProcessor {
    // Where the command requests should be sent
    target: StreamTarget,
}

impl CommandProcessor {
    ///
    /// Creates a new command processor that will send commands to the specified target
    ///
    pub fn new(target: impl Into<StreamTarget>) -> Self {
        CommandProcessor {
            target: target.into()
        }
    }

    ///
    /// Runs a command, returning the response
    ///
    pub async fn run_command(&self, command: CommandName, parameter: serde_json::Value, context: &SceneContext) -> CommandResponse {
        // Retrieve the target for the commands
        let target = self.target.clone();

        // Create the command query
        let command = JsonCommand::new((), command, parameter);

        // Run the command and retrieve the first response if we can
        let command_result = context.spawn_query(ReadCommand::default(), command, target);

        match command_result {
            Err(err) => CommandResponse::Error(format!("Could not send command: {:?}", err)),

            Ok(mut result_stream) => {
                // If the command returns more than one response, we combine all of the JSON into a single JSON response
                let mut json = vec![];

                while let Some(next_response) = result_stream.next().await {
                    match next_response {
                        CommandResponse::Json(response_json) => {
                            // Combine all the JSON responses into a single one
                            json.extend(response_json)
                        }

                        CommandResponse::Error(err) => {
                            // For an error response, just return that as the only response
                            return CommandResponse::Error(err);
                        }
                    }
                }

                CommandResponse::Json(json)
            }
        }
    }
}

impl Command for CommandProcessor {
    type Input  = Result<CommandRequest, ()>;
    type Output = CommandResponse;

    fn run<'a>(&'a self, input: impl 'static + Send + Stream<Item=Self::Input>, context: SceneContext) -> impl 'a + Send + Future<Output=()> {
        async move {
            pin_mut!(input);
            let mut responses   = context.send::<CommandResponse>(()).unwrap();

            while let Some(next_command) = input.next().await {
                use CommandRequest::*;

                let response = match next_command {
                    Ok(Command     { command, argument }) => { self.run_command(command, argument, &context).await }
                    Ok(Pipe        { from, to })          => { CommandResponse::Error("Not implemented yet".into()) }
                    Ok(Assign      { variable, from })    => { CommandResponse::Error("Not implemented yet".into()) }
                    Ok(ForTarget   { target, request })   => { CommandResponse::Error("Not implemented yet".into()) }
                    Err(_)                                => { CommandResponse::Error("Not implemented yet".into()) }
                };

                if responses.send(response).await.is_err() {
                    break;
                }
            }
        }
    }
}

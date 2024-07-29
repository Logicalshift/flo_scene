use super::command_stream::*;
use super::command_socket::*;
use super::json_command::*;
use crate::socket::*;

use flo_scene::*;
use flo_scene::commands::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use std::iter;
use std::sync::*;

///
/// A connection to a simple command program
///
/// The simple command program can just read and write command responses, and cannot provide direct access to the terminal
///
pub type CommandProgramSocketMessage = SocketMessage<CommandData, CommandData>;

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
                let socket = CommandSocket::connect(connection);

                // Spawn a reader for the command input
                context.spawn_command(CommandProcessor::new(socket, command_target.clone()), stream::empty()).ok();
            }
        }
    }
}

///
/// The command processor command, which takes an input of parsed commands, and generates the corresponding responses
///
/// This will generate one response per command
///
#[derive(Clone)]
pub struct CommandProcessor {
    // The command socket connection (or none if the command is running)
    socket: Arc<Mutex<Option<CommandSocket>>>,

    // The target where the commands should be run
    target: StreamTarget,
}

impl CommandProcessor {
    ///
    /// Creates a new command processor that will send commands to the specified target
    ///
    pub fn new(socket: CommandSocket, target: StreamTarget) -> Self {
        let socket = Arc::new(Mutex::new(Some(socket)));
        CommandProcessor { socket, target }
    }

    ///
    /// Runs a command, returning the response
    ///
    pub async fn run_command(&self, command: CommandName, parameter: serde_json::Value, context: &SceneContext) -> BoxStream<'static, CommandResponse> {
        // Retrieve the target for the commands
        let target = self.target.clone();

        // Create the command query
        let command = JsonCommand::new((), command, parameter);

        // Run the command and retrieve the first response if we can
        let command_result = context.spawn_query(ReadCommand::default(), command, target);

        match command_result {
            Err(err)            => stream::iter(iter::once(CommandResponse::Error(format!("Could not send command: {:?}", err)))).boxed(),
            Ok(result_stream)   => result_stream.boxed()
        }
    }
}

impl Command for CommandProcessor {
    type Input  = ();
    type Output = ();

    fn run<'a>(&'a self, _input: impl 'static + Send + Stream<Item=Self::Input>, context: SceneContext) -> impl 'a + Send + Future<Output=()> {
        // Take the socket from inside the object
        let mut socket = self.socket.lock().unwrap().take().unwrap();

        async move {
            while let Ok(next_command) = socket.next_request().await {
                use CommandRequest::*;

                // Read the next command and decide on the response
                let command_responses = match next_command {
                    Command     { command, argument } => { self.run_command(command, argument, &context).await }
                    Pipe        { from, to }          => { stream::iter(iter::once(CommandResponse::Error("Not implemented yet".into()))).boxed() }
                    Assign      { variable, from }    => { stream::iter(iter::once(CommandResponse::Error("Not implemented yet".into()))).boxed() }
                    ForTarget   { target, request }   => { stream::iter(iter::once(CommandResponse::Error("Not implemented yet".into()))).boxed() }
                };

                // Send the responses to the socket
                if socket.send_responses(command_responses).await.is_err() {
                    break;
                }
            }
        }
    }
}

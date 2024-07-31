use super::command_stream::*;
use super::command_socket::*;
use super::json_command::*;
use crate::socket::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::{pin_mut};
use futures::stream::{BoxStream};
use once_cell::sync::{Lazy};

use std::collections::{HashMap};
use std::iter;
use std::sync::*;

/// Filter that maps the 'Query' message to a CommandSessionRequest message
static COMMAND_SESSION_VARIABLE_QUERY_FILTER: Lazy<FilterHandle> = Lazy::new(|| FilterHandle::for_filter(|stream: InputStream<Query<CommandVariable>>| stream.map(|msg| CommandSessionRequest::QueryAllVariables(msg.target()))));

///
/// A connection to a simple command program
///
/// The simple command program can just read and write command responses, and cannot provide direct access to the terminal
///
pub type CommandProgramSocketMessage = SocketMessage<CommandData, CommandData>;

///
/// Requests that can be made to an active command session
///
/// This is the message type accepted by the subprograms started by the `command_connection_program` subprogram
///
#[derive(Clone, Debug, PartialEq)]
pub enum CommandSessionRequest {
    /// Changes a variable in this session
    SetVariable(String, serde_json::Value),

    /// Queries a variable, sending a `QueryResponse<CommandVariable>` response to the specified target
    QueryVariable(String, StreamTarget),

    /// As for QueryVariable, except sends the values of all of the variables to the specified target as `QueryResponse<CommandVariable>` messages
    QueryAllVariables(StreamTarget),
}

///
/// Query response indicating the value of a variable in a command session
///
#[derive(Clone, Debug, PartialEq)]
pub struct CommandVariable(pub String, pub serde_json::Value);

impl SceneMessage for CommandSessionRequest {
    fn initialise(scene: &Scene) {
        scene.connect_programs(StreamSource::Filtered(*COMMAND_SESSION_VARIABLE_QUERY_FILTER), (), StreamId::with_message_type::<Query<CommandVariable>>()).unwrap();
    }
}

impl SceneMessage for CommandVariable { }

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

    // Spawn session tasks for each connection
    let mut input = input;
    while let Some(connection) = input.next().await {
        match connection {
            SocketMessage::Connection(connection) => {
                // Connect the command socket
                let socket          = CommandSocket::connect(connection);
                let command_target  = command_target.clone();

                // Spawn a subprogram to handle running the commands using the CommandSession
                let command_session_id = SubProgramId::new();
                context.send_message(SceneControl::start_program(
                    command_session_id,
                    move |input, context| async move {
                        let command_session = CommandSession::new(socket, command_target);
                        command_session.run(input, context).await;
                    },
                    0)).await.ok();
            }
        }
    }
}

///
/// The command session reads commands from a socket and evaluates them
///
#[derive(Clone)]
pub struct CommandSession {
    /// The command socket connection (or none if the command is running)
    socket: Arc<Mutex<Option<CommandSocket>>>,

    /// The target where the commands should be run
    target: StreamTarget,

    /// The variables for this command session
    variables: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl CommandSession {
    ///
    /// Creates a new command processor that will send commands to the specified target
    ///
    pub fn new(socket: CommandSocket, target: StreamTarget) -> Self {
        let socket = Arc::new(Mutex::new(Some(socket)));
        let variables = Arc::new(Mutex::new(HashMap::new()));
        CommandSession { socket, target, variables }
    }

    ///
    /// Runs a command, returning the response
    ///
    pub async fn run_command(&self, command: CommandName, parameter: serde_json::Value, context: &SceneContext) -> BoxStream<'static, CommandResponse> {
        // Retrieve the target for the commands
        let target = self.target.clone();

        // Create the command query
        let command = JsonCommand::new((), command, parameter, context.current_program_id());

        // Run the command and retrieve the first response if we can
        let command_result = context.spawn_query(ReadCommand::default(), command, target);

        match command_result {
            Err(err)            => stream::iter(iter::once(CommandResponse::Error(format!("Could not send command: {:?}", err)))).boxed(),
            Ok(result_stream)   => result_stream.boxed()
        }
    }

    ///
    /// Runs the command session program
    ///
    pub fn run<'a>(&'a self, input: impl 'static + Send + Stream<Item=CommandSessionRequest>, context: SceneContext) -> impl 'a + Send + Future<Output=()> {
        // Set up the session state
        let run_variables   = Arc::clone(&self.variables);
        let input_variables = Arc::clone(&self.variables);
        let run_context     = context;
        let input_context   = run_context.clone();

        // Take the socket from inside the object
        let mut socket = self.socket.lock().unwrap().take().unwrap();

        // Create a future that runs the commands received from the socket
        let run_commands = async move {
            let context     = run_context;
            let variables   = run_variables;

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
        };

        // Create another future that processes command requests
        let process_input = async move {
            let variables   = input_variables;
            let context     = input_context;

            pin_mut!(input);
            while let Some(request) = input.next().await {
                match request {
                    CommandSessionRequest::SetVariable(name, value) => {
                        // Just set the variable immediately
                        variables.lock().unwrap().insert(name, value);
                    }

                    CommandSessionRequest::QueryVariable(name, target) => {
                        // Read the variable value; we'll use null if the variable is not set
                        let value = variables.lock().unwrap().get(&name).cloned();
                        let value = value.unwrap_or(serde_json::Value::Null);

                        // Send the value as a query response
                        if let Ok(mut target) = context.send(target) {
                            target.send(QueryResponse::with_data(CommandVariable(name, value))).await.ok();
                        }
                    }

                    CommandSessionRequest::QueryAllVariables(target) => {
                        // Read all the variable values
                        let values = variables.lock().unwrap().iter()
                            .map(|(name, value)| CommandVariable(name.clone(), value.clone()))
                            .collect::<Vec<_>>();

                        // Send the list as a query response
                        if let Ok(mut target) = context.send(target) {
                            target.send(QueryResponse::with_iterator(values)).await.ok();
                        }
                    }
                }
            }
        };

        // The session runs until either of the two futures terminates
        future::select(Box::pin(run_commands), Box::pin(process_input))
            .map(|_| ())
    }
}

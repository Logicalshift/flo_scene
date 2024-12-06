use super::command_stream::*;
use super::command_socket::*;
use super::json_command::*;
use crate::socket::*;
use crate::parse_json::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use flo_stream::*;

use futures::prelude::*;
use futures::{pin_mut};
use futures::future::{BoxFuture};
use futures::stream::{BoxStream};
use futures::channel::mpsc;
use once_cell::sync::{Lazy};
use serde::*;

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
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
pub struct CommandVariable(pub String, pub serde_json::Value);

impl SceneMessage for CommandSessionRequest {
    fn initialise(scene: &Scene) {
        scene.connect_programs(StreamSource::Filtered(*COMMAND_SESSION_VARIABLE_QUERY_FILTER), (), StreamId::with_message_type::<Query<CommandVariable>>()).unwrap();
    }

    #[inline]
    fn message_type_name() -> String { "flo_scene_pipe::CommandSessionRequest".into() }
}

impl SceneMessage for CommandVariable { 
    #[inline]
    fn message_type_name() -> String { "flo_scene_pipe::CommandVariable".into() }
}

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
    /// Substitutes any variables found in a `ParsedJson` structure.
    ///
    /// Return value is the substituted variable or an error response
    ///
    pub fn substitute_variables<'a>(&'a self, parsed_json: ParsedJson, context: &'a SceneContext) -> BoxFuture<'a, Result<serde_json::Value, CommandResponse>> {
        // TODO: a way to have `command { }` type substitutions
        async move {
            use ParsedJson::*;
            use serde_json::Value;

            match parsed_json {
                Null                => Ok(Value::Null),
                Bool(val)           => Ok(Value::Bool(val)),
                Number(num)         => Ok(Value::Number(num)),
                String(string)      => Ok(Value::String(string)),

                Array(array) => {
                    let mut substituted = Vec::with_capacity(array.len());

                    for val in array {
                        substituted.push(self.substitute_variables(val, context).await?);
                    }

                    Ok(Value::Array(substituted))
                }

                Object(map) => {
                    let mut substituted = serde_json::Map::new();

                    for (key, val) in map {
                        let val = self.substitute_variables(val, context).await?;
                        substituted.insert(key, val);
                    }

                    Ok(Value::Object(substituted))
                }

                Variable(variable) => {
                    if let Some(value) = self.variables.lock().unwrap().get(&variable).cloned() {
                        Ok(value)
                    } else {
                        Err(CommandResponse::Error(format!("Variable '{}' is not defined", variable)))
                    }
                }
            }
        }.boxed()
    }

    ///
    /// Evaluates a command request in this session
    ///
    pub fn evaluate_request<'a>(&'a self, request: CommandRequest, context: &'a SceneContext) -> BoxFuture<'a, BoxStream<'a, CommandResponse>> {
        async move {
            use CommandRequest::*;

            match request {
                Command     { command, argument } => { self.run_command(command, argument, &context).await }
                RawJson     { value }             => { self.raw_json(value, context).await }
                Pipe        { from, to }          => { self.pipe(from, to, context).await }
                Assign      { variable, from }    => {
                    let request_responses = self.evaluate_request(*from, context).await;
                    self.assign(variable, request_responses).await
                }
                ForTarget   { target, request }   => { self.for_target(target, request, context).await }
            }
        }.boxed()
    }

    ///
    /// Runs a command, returning the response
    ///
    pub async fn run_command<'a>(&'a self, command: CommandName, parameter: ParsedJson, context: &SceneContext) -> BoxStream<'a, CommandResponse> {
        let parameter = match self.substitute_variables(parameter, context).await {
            Ok(json) => json,
            Err(err) => { return stream::iter(iter::once(err)).boxed(); }
        };

        // Retrieve the target for the commands
        let target = self.target.clone();

        // Check for a variable matching this command name
        let variable_value = {
            let variables                   = self.variables.lock().unwrap();
            let CommandName(command_name)   = &command;
            variables.get(command_name).cloned()
        };

        if let Some(variable_value) = variable_value {
            // Variables replace commands (even with parameters), so if a variable is defined, this is the value
            stream::iter(iter::once(CommandResponse::Json(variable_value))).boxed()
        } else {
            // Create the command query
            let command = JsonCommand::new((), command, parameter, context.current_program_id());

            // Run the command and retrieve the first response if we can
            let command_result = context.spawn_query(ReadCommand::default(), command, target);

            match command_result {
                Err(err)            => stream::iter(iter::once(CommandResponse::Error(format!("Could not send command: {:?}", err)))).boxed(),
                Ok(result_stream)   => result_stream.boxed()
            }
        }
    }

    ///
    /// Evaluates a raw JSON request
    ///
    async fn raw_json(&self, value: ParsedJson, context: &SceneContext) -> BoxStream<'_, CommandResponse> {
        match self.substitute_variables(value, context).await {
            Ok(json) => stream::iter(iter::once(CommandResponse::Json(json))).boxed(),
            Err(err) => stream::iter(iter::once(err)).boxed(),
        }
    }

    ///
    /// Pipes the output of one command to the input of another
    ///
    async fn pipe<'a>(&'a self, from: Box<CommandRequest>, to: Box<CommandRequest>, context: &'a SceneContext) -> BoxStream<'a, CommandResponse> {
        // Run both commands
        let run_first_command   = self.evaluate_request(*from, context);
        let run_second_command  = self.evaluate_request(*to, context);

        // Wait for them to produce their responses. A pipe sends either JSON values or background streams from the first command to the second
        let (in_responses, out_responses) = future::join(run_first_command, run_second_command).await;

        // Result is a generator stream
        generator_stream(move |yield_value| async move {
            // From the 'in' stream we either want the JSON value or the background stream, if there is one
            let mut in_responses    = in_responses;
            let mut out_responses   = out_responses;

            // Process the 'output' command responses first, looking for a stream to send the output of the first command
            let mut io_stream = None;
            while let Some(out_response) = out_responses.next().await {
                match out_response {
                    CommandResponse::IoStream(new_io_stream) => {
                        // Keep the IO stream to use with the results from the input
                        io_stream = Some(new_io_stream);
                    }

                    // Other values are sent to the output
                    other => yield_value(other).await
                }
            }

            let io_stream = if let Some(io_stream) = io_stream { 
                io_stream 
            } else {
                // Stop processing if no IO stream is generated (responses from the first command are lost)
                yield_value(CommandResponse::Message(format!("Pipe target command has no input stream"))).await;
                return;
            };

            // Start the IO stream
            let (send, recv) = mpsc::channel(1);
            let piped_output = io_stream(recv.boxed());

            // Behaviour of the 'input' command depends on if there's a JSON response or a background stream first
            // (We send from the background stream if that's first, or just JSON responses otherwise, so either style of commands can be piped)
            let mut sent_json               = false;
            let mut sent_from_background    = false;
            let mut send                    = send;
            let mut piped_output            = piped_output;

            future::join(
                async {
                    // TODO: we should stop reading from the input if the pipe output is closed (as there'll be nowhere to send it)
                    // (Although, provided that the following 'send' fails we'll generally stop at the next message anyway)
                    while let Some(in_response) = in_responses.next().await {
                        match in_response {
                            CommandResponse::Json(json_value) => {
                                // We send JSON data if there's no background stream
                                // (If a background stream follows the first JSON message, it's swallowed)
                                if !sent_from_background {
                                    sent_json = true;

                                    if send.send(json_value).await.is_err() {
                                        // Stop processing if there's an error sending to the output
                                        break;
                                    }
                                }
                            }

                            CommandResponse::BackgroundStream(background_stream) => {
                                // Background streams are swallowed if we've already sent any data
                                if !sent_json && !sent_from_background {
                                    // Send the data from the stream to the pipe
                                    sent_from_background = true;

                                    let mut background_stream = background_stream;
                                    while let Some(json_value) = background_stream.next().await {
                                        if send.send(json_value).await.is_err() {
                                            // Stop processing if there's an error sending to the output
                                            // (We'll keep processing the rest of the output from the 'in' side)
                                            break;
                                        }
                                    }
                                }

                                if sent_json {
                                    yield_value(CommandResponse::Error("Background streams must be sent first to use with a pipe".into())).await;
                                    break;
                                }
                            }

                            // Everything else goes to the output
                            other => yield_value(other).await,
                        }
                    }
                }.boxed(),
                async {
                    // Output anything the pipe target produces as a JSON value in the response
                    while let Some(output) = piped_output.next().await {
                        yield_value(CommandResponse::Json(output)).await;
                    }
                }.boxed()).await;

            // We need an IoStream from the 'out' stream
        }).boxed()
    }

    ///
    /// Sends the responses to a command to a stream target, as a `QueryResponse<CommandResponse>` value.
    ///
    /// This is generally for where we want to evaluate commands using the query API, `ForTarget` requests are generated by calling `with_new_target()` on
    /// a `CommandRequest`.
    ///
    fn for_target<'a>(&'a self, target: StreamTarget, request: Box<CommandRequest>, context: &'a SceneContext) -> BoxFuture<'a, BoxStream<'a, CommandResponse>> {
        async move {
            generator_stream(move |yield_value| async move {
                // Send responses for the command to the requested target
                if let Ok(stream_target) = context.send(target.clone()) {
                    yield_value(CommandResponse::Message(format!("Sending to {:?}", target))).await;

                    // Evaluate the request
                    let request_responses = self.evaluate_request(*request, context).await;

                    // The responses we get are limited in life to 'a, so we need to copy them away to send to the target
                    yield_value(CommandResponse::Message("Reading responses".into())).await;
                    let request_responses = request_responses.collect::<Vec<_>>().await;

                    // Pass on to the stream target
                    let mut stream_target = stream_target;
                    stream_target.send(QueryResponse::with_stream(stream::iter(request_responses))).await.ok();

                    yield_value(CommandResponse::Message(format!("Result sent to {:?}", target))).await;
                } else {
                    yield_value(CommandResponse::Error(format!("Could not connect to {:?}", target))).await;
                };
            }).boxed()
        }.boxed()
    }

    ///
    /// Assigns the result of a response stream to a variable, returning a stream of results to pass on to the user or the next stage
    ///
    /// There are two types of response that can be assigned to a variable:
    ///
    ///   * A JSON result will just assign that value straight to the variable
    ///   * A JSON stream will initially assign 'null' to the variable and then assign whatever is the most recent message to the variable (so this can be used to 
    ///     represent an updating state). A message is generated to indicate that this has happened.
    ///
    /// Errors will short-circuit the assignment (ie, we'll display the error and any results will be left out)
    ///
    pub async fn assign<'a>(&'a self, variable: impl Into<String>, response: BoxStream<'a, CommandResponse>) -> BoxStream<'a, CommandResponse> {
        let variable = variable.into();

        // The assignment happens when the response reader reaches the appropriate point
        generator_stream(move |yield_value| async move {
            let mut responses = response;

            // Read until we can assign a variable
            loop {
                let response = responses.next().await;

                match response {
                    Some(CommandResponse::Json(value)) => {
                        // Assign this value to the variable
                        yield_value(CommandResponse::Message(format!("Result assigned to `{}`", variable))).await;
                        self.variables.lock().unwrap().insert(variable, value);
                        break;
                    }

                    Some(CommandResponse::Error(err)) => {
                        // If an error is generated before we get an assignment to make, 
                        yield_value(CommandResponse::Error(err)).await;
                        break;
                    }

                    Some(response) => {
                        // Default behaviour is to yield the response and carry out
                        yield_value(response).await;
                    }

                    None => {
                        // No value to assign: report an error and abort
                        yield_value(CommandResponse::Error("Command did not generate a value that can be assigned to this variable".into())).await;
                        return;
                    }
                }
            }

            // The variable is assigned or the assignment was aborted: all other responses are yielded directly
            while let Some(response) = responses.next().await {
                yield_value(response).await;
            }
        }).boxed()
    }

    ///
    /// Runs the command session program
    ///
    pub fn run<'a>(&'a self, input: impl 'static + Send + Stream<Item=CommandSessionRequest>, context: SceneContext) -> impl 'a + Send + Future<Output=()> {
        // Set up the session state
        let input_variables = Arc::clone(&self.variables);
        let run_context     = context;
        let input_context   = run_context.clone();

        // Take the socket from inside the object
        let mut socket = self.socket.lock().unwrap().take().unwrap();

        // Create a future that runs the commands received from the socket
        let run_commands = async move {
            let context     = run_context;

            while let Ok(next_command) = socket.next_request().await {
                // Read the next command and decide on the response
                let command_responses = self.evaluate_request(next_command, &context).await;

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

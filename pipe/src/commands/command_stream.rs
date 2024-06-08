use crate::parser::*;

use flo_scene::programs::QueryRequest;
use flo_scene::*;

use futures::prelude::*;
use futures::{pin_mut};
use futures::future::{BoxFuture};
use futures::stream::{BoxStream};

use serde::{Deserialize, Serialize};
use serde_json;
use flo_stream::{generator_stream};

use std::task::{Poll};

///
/// A string value representing the name of a command sent to a stream
///
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CommandName(pub String);

///
/// A string value representing the name of a variable to assign
///
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VariableName(pub String);

///
/// An argument to a command sent to a stream
///
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandArgument {
    Json(serde_json::Value)
}

///
/// A command parsed from an input stream
///
/// Commands have the format `<CommandName> <Argument>`, where the command name is an identifier and the arguments is a single
/// JSON value (multiple values can be passed by chained together commands using '|' operator)
///
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandRequest {
    Command     { command: CommandName, argument: serde_json::Value },
    Pipe        { from: Box<CommandRequest>, to: Box<CommandRequest> },
    Assign      { variable: VariableName, from: Box<CommandRequest> },
    ForTarget   { target: StreamTarget, request: Box<CommandRequest> }
}

///
/// Possible responses from a command
///
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandResponse {
    /// A stream of JSON values
    Json(Vec<serde_json::Value>),

    /// An error message
    Error(String),    
}

impl SceneMessage for CommandRequest { }
impl SceneMessage for CommandResponse { }

impl QueryRequest for CommandRequest {
    type ResponseData = CommandResponse;

    fn with_new_target(self, new_target: StreamTarget) -> Self {
        match self {
            CommandRequest::ForTarget { request, .. } => {
                CommandRequest::ForTarget { target: new_target, request: request }
            }

            other => {
                CommandRequest::ForTarget { target: new_target, request: Box::new(other) }
            }
        }
    }
}

impl CommandRequest {
    ///
    /// Creates a command by parsing a string
    ///
    pub async fn parse(command: &str) -> Result<CommandRequest, ()> {
        let mut parser      = Parser::new();
        let mut tokenizer   = Tokenizer::new(stream::iter(command.bytes()).ready_chunks(256));

        tokenizer.with_command_matchers();

        command_parse(&mut parser, &mut tokenizer).await?;

        Ok(parser.finish().map_err(|_| ())?)
    }
}

///
/// Reads an input stream containing commands in text form and outputs the command structures as they are matched
///
/// This can be used as the input side of a socket
///
/// Commands are relatively simple, they have the structure `<name> <parameters>` where the name is an identifier (containing alphanumeric characters, 
/// alongside '_', '.' and ':'). Parameters are just JSON values, and commands are ended by a newline character that is outside of a JSON value.
///
pub fn parse_command_stream(input: impl 'static + Send + Unpin + Stream<Item=Vec<u8>>) -> impl 'static + Send + Unpin + Stream<Item=Result<CommandRequest, ()>> {
    generator_stream(move |yield_value| async move {
        let mut tokenizer   = Tokenizer::new(input);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        // TODO: loop until EOF
        loop {
            // Read the next command
            let next_command = command_parse(&mut parser, &mut tokenizer).await;

            match next_command {
                Ok(()) => {
                    // Finish the parse and continue with the next command
                    let command = parser.finish().map_err(|_| ());
                    yield_value(command).await;
                }

                Err(()) => {
                    // Throw away the contents of the parser
                    parser.abort();

                    // TODO: Discard tokens until we encounter a newline

                    // TODO: parse until EOF
                    break;
                }
            }
        }
    })
}

///
/// Displays the result of a command
///
async fn display_response(yield_value: &(impl Send + Fn(String) -> BoxFuture<'static, ()>), response: CommandResponse) {
    match response {
        CommandResponse::Json(json) => {
            // Format the JSON as a pretty-printed string (TODO: the to_writer_pretty version would be better for very long JSON)
            let json_string = serde_json::to_string_pretty(&json);
            yield_value(format!("{:?}\n", json_string)).await;
        },

        CommandResponse::Error(error_message) => {
            // '!!! <error>' if there's a problem
            yield_value(format!("!!! {:?}\n", error_message)).await;
        }
    }
}

///
/// Displays the output of the responses to a set of commands as a stream of UTF-8 data
///
/// This can be used as the output side of a socket
///
pub fn display_command_responses(input: impl 'static + Send + Unpin + Stream<Item=CommandResponse>) -> BoxStream<'static, Vec<u8>> {
    // The way we generate the responses and prompts is to generate strings and then convert them into bytes later on
    generator_stream::<String, _, _>(|yield_value| async move {
        pin_mut!(input);

        // We always start by showing a prompt for the next command
        yield_value("\n\n> ".into()).await;

        'main_loop: loop {
            // Process until the input is exhuasted
            match input.next().await {
                None => {
                    // No more input
                    break; 
                }

                Some(response) => {
                    // Display the response
                    yield_value("\n".into()).await;
                    display_response(&yield_value, response).await;

                    // Poll the input future for more responses if there are any waiting immediately
                    while let Ok(next_response) = future::poll_fn(|context| {
                        match input.poll_next_unpin(context) {
                            Poll::Ready(result) => Poll::Ready(Ok(result)),
                            Poll::Pending       => Poll::Ready(Err(()))
                        }
                    }).await {
                        match next_response {
                            Some(response) => {
                                yield_value("\n".into()).await;
                                display_response(&yield_value, response).await;
                            }

                            None => { break 'main_loop; }
                        }
                    }
                }
            }

            // Display a prompt once input is no longer being generated
            yield_value("\n> ".into()).await;
        }

        // Sign out
        yield_value("\n\n.\n".into()).await;
    }).map(|string| string.into_bytes()).boxed()
}

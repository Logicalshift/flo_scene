use crate::parser::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::{pin_mut};
use futures::future::{BoxFuture};
use futures::stream::{BoxStream};
use futures::channel::mpsc;

use serde::{Deserialize, Serialize};
use serde_json;
use flo_stream::{generator_stream};

use std::collections::{VecDeque};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::iter;
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
pub enum CommandResponse {
    /// A commentary message, written as '  <message>'
    Message(String),

    /// A JSON value, written out directly
    Json(serde_json::Value),

    /// A stream of values that can be outputted at any time, used for receiving monitored events
    /// A new stream is given a number in the initial response using a message of format '<<< <n>' (eg, '<<< 8')
    /// Events from that stream are displayed as '<<n> <json>', eg '<8 [ 1, 2, 3, 4 ]' - note that the JSON can
    /// spread across several lines. When the stream is closed, a '<EOS <n>' message is generated.
    BackgroundStream(BoxStream<'static, serde_json::Value>),

    /// An error message, written as '!!! <error>'
    Error(String),    
}

impl SceneMessage for CommandRequest { }
impl SceneMessage for CommandResponse { }

impl Into<String> for CommandName {
    #[inline]
    fn into(self) -> String {
        self.0
    }
}

impl From<CommandError> for CommandResponse {
    fn from(err: CommandError) -> Self {
        CommandResponse::Error(format!("{:?}", err))
    }
}

impl From<ListCommandResponse> for CommandResponse {
    fn from(list_response: ListCommandResponse) -> Self {
        CommandResponse::Json(list_response.serialize(serde_json::value::Serializer).unwrap())
    }
}

impl TryInto<ListCommandResponse> for CommandResponse {
    type Error = CommandError;

    fn try_into(self) -> Result<ListCommandResponse, CommandError> {
        match self {
            CommandResponse::Json(json) => {
                ListCommandResponse::deserialize(json)
                    .map_err(|_| CommandError::CannotConvertResponse)
            }

            // Other types of response cannot be JSON requests
            _ => Err(CommandError::CannotConvertResponse)
        }
    }
}

impl Debug for CommandResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CommandResponse::Message(msg)           => write!(f, "Message({:?})", msg),
            CommandResponse::Json(json)             => write!(f, "Json({:?})", json),
            CommandResponse::BackgroundStream(_)    => write!(f, "BackgroundStream(...)"),
            CommandResponse::Error(err)             => write!(f, "Error({:?})", err),
        }
    }
}

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
async fn display_response(yield_value: &(impl Send + Fn(String) -> BoxFuture<'static, ()>), put_stream_in_background: &mut (impl Send + Unpin + Sink<BoxStream<'static, serde_json::Value>>), response: CommandResponse) {
    match response {
        CommandResponse::Message(msg) => {
            let msg = msg.replace("\n", "\n  ");
            yield_value(format!("  {}\n", msg)).await;
        }

        CommandResponse::Json(json) => {
            // Format the JSON as a pretty-printed string (TODO: the to_writer_pretty version would be better for very long JSON)
            let json_string = serde_json::to_string_pretty(&json);

            if let Ok(json_string) = json_string {
                yield_value(format!("{}\n", json_string)).await;
            } else {
                yield_value(format!("!!! {:?}\n", "Could not format JSON response")).await;
            }
        },

        CommandResponse::BackgroundStream(stream) => {
            // This requires moving the stream to the background
            put_stream_in_background.send(stream).await.ok();
        },

        CommandResponse::Error(error_message) => {
            // '!!! <error>' if there's a problem
            yield_value(format!("!!! {}\n", error_message)).await;
        }
    }
}

///
/// A display request is used as the internal message type for receiving command responses or messages from background streams
///
enum DisplayRequest {
    /// Standard command response
    CommandResponse(CommandResponse),

    /// A new background stream was created
    NewBackgroundStream(usize),

    /// A background stream was closed
    ClosedBackgroundStream(usize),

    /// A message was received from one of the background streams
    StreamMessage(usize, serde_json::Value),

    /// Finishes the display stream
    Stop,
}

///
/// Creates a stream that multiplexes background streams and writes to the output
///
fn background_command_streams() -> (impl 'static + Send + Unpin + Stream<Item=DisplayRequest>, impl 'static + Send + Unpin + Sink<BoxStream<'static, serde_json::Value>, Error=mpsc::SendError>) {
    // Create the channel where new background streams can be sent
    let (send_new_streams, new_streams) = mpsc::channel::<BoxStream<'static, serde_json::Value>>(1);

    // The stream we return reads from any stream passed in to the new streams list
    // TODO: this isn't very efficient (fine for small numbers of streams but we should probably use a context that only polls the streams that are needed)
    let mut monitored_streams   = VecDeque::new();
    let mut maybe_new_streams   = Some(new_streams);
    let mut next_stream_num     = 0;

    let stream = stream::poll_fn(move |context| {
        // Add any new streams from the new_streams stream
        if let Some(new_streams) = &mut maybe_new_streams {
            match new_streams.poll_next_unpin(context) {
                Poll::Pending                   => { }
                Poll::Ready(None)               => { maybe_new_streams = None;}
                Poll::Ready(Some(new_stream))   => { 
                    let stream_num = next_stream_num;
                    next_stream_num += 1;

                    monitored_streams.push_back((stream_num, new_stream));

                    // Generates a 'new background stream' message
                    return Poll::Ready(Some(DisplayRequest::NewBackgroundStream(stream_num)));
                }
            }
        }

        // Poll all of the monitored streams until we get a response, or we find they're all pending
        // The VecDeque rotation we do here ensures that if there is one very active stream it can't drown out the others
        let num_streams = monitored_streams.len();

        for _ in 0..num_streams {
            // Take the next stream from the rotation
            if let Some((stream_num, mut next_stream)) = monitored_streams.pop_front() {
                // Check for output
                match next_stream.poll_next_unpin(context) {
                    Poll::Ready(Some(value)) => {
                        // Return the stream to the list
                        monitored_streams.push_back((stream_num, next_stream));

                        // Generate a result for this stream
                        return Poll::Ready(Some(DisplayRequest::StreamMessage(stream_num, value)));
                    }

                    Poll::Ready(None) => {
                        // Don't return this stream to the list and indicate that it's finished
                        return Poll::Ready(Some(DisplayRequest::ClosedBackgroundStream(stream_num)));
                    }

                    Poll::Pending => {
                        // Return the stream to the end of the list to poll next time around
                        monitored_streams.push_back((stream_num, next_stream));
                    }
                }
            }
        }

        // No new streams and everything else is pending
        Poll::Pending
    });

    (stream, send_new_streams)
}

///
/// Displays the output of the responses to a set of commands as a stream of UTF-8 data
///
/// This can be used as the output side of a socket
///
pub fn display_command_responses(input: impl 'static + Send + Unpin + Stream<Item=CommandResponse>) -> BoxStream<'static, Vec<u8>> {
    // The way we generate the responses and prompts is to generate strings and then convert them into bytes later on
    generator_stream::<String, _, _>(|yield_value| async move {
        let (background_messages, background_stream_sender) = background_command_streams();

        let input = input.map(|response| DisplayRequest::CommandResponse(response)).chain(stream::iter(iter::once(DisplayRequest::Stop)));
        let input = stream::select(input, background_messages); // TODO: need to close the whole stream when input is closed, as background_messages will last indefinitely

        pin_mut!(input);

        // We always start by showing a prompt for the next command
        yield_value("\n\n> ".into()).await;

        let mut background_stream_sender = background_stream_sender;

        'main_loop: loop {
            // Process until the input is exhuasted
            match input.next().await {
                None => {
                    // No more input
                    break;
                }

                Some(mut response) => {
                    'process_responses: loop {
                        // Display the response
                        match response {
                            DisplayRequest::CommandResponse(response) => {
                                yield_value("\n".into()).await;
                                display_response(&yield_value, &mut background_stream_sender, response).await;
                            }

                            DisplayRequest::NewBackgroundStream(stream_num) => {
                                yield_value(format!("\n<<< {}\n", stream_num)).await;
                            }

                            DisplayRequest::ClosedBackgroundStream(stream_num) => {
                                yield_value(format!("\n<EOS {}\n", stream_num)).await;
                            }

                            DisplayRequest::StreamMessage(stream_num, msg) => {
                                if let Ok(json) = serde_json::to_string_pretty(&msg) {
                                    yield_value(format!("\n<{} {}\n", stream_num, json)).await;
                                }
                            }

                            DisplayRequest::Stop => {
                                break 'main_loop;
                            }
                        }

                        // Read the next response immediately if one is waiting
                        let next_response = future::poll_fn(|context| {
                            match input.poll_next_unpin(context) {
                                Poll::Ready(result) => Poll::Ready(Ok(result)),
                                Poll::Pending       => Poll::Ready(Err(()))
                            }
                        }).await;

                        response = match next_response {
                            Ok(Some(response))  => response,
                            Ok(None)            => { break 'main_loop; }
                            Err(())             => { break 'process_responses; }
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

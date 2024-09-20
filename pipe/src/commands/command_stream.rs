use super::command_socket::*;
use crate::parser::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use serde::{Deserialize, Serialize};
use serde_json;

use std::fmt;
use std::fmt::{Debug, Formatter};

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
    Command     { command: CommandName, argument: ParsedJson },
    RawJson     { value: ParsedJson },
    Pipe        { from: Box<CommandRequest>, to: Box<CommandRequest> },
    Assign      { variable: VariableName, from: Box<CommandRequest> },
    ForTarget   { target: StreamTarget, request: Box<CommandRequest> },
}

///
/// The responses that a JSON command can provide
///
/// Commands can provide data as JSON values using the `CommandResponse::Json` response type. This is the
/// usual type of response and the main type that is acted upon by other commands.
///
/// It is also possible to generate messages for the user by using the `Message` response type. These are
/// just informational and can be ignored by things that want to process the data generated by commands.
///
/// Commands can also generate a background stream of data. This is useful if they are monitoring some kind
/// of event.
///
pub enum CommandResponse {
    /// A JSON value representing the data generated by this command. This is the data that is parsed by other commands if that's what they are doing.
    Json(serde_json::Value),

    /// A commentary message, written as '  <message>'
    Message(String),

    /// A stream of values that can be outputted at any time, used for receiving monitored events
    /// A new stream is given a number in the initial response using a message of format '<<< <n>' (eg, '<<< 8')
    /// Events from that stream are displayed as '<<n> <json>', eg '<8 [ 1, 2, 3, 4 ]' - note that the JSON can
    /// spread across several lines. When the stream is closed, a '<EOS <n>' message is generated.
    BackgroundStream(BoxStream<'static, serde_json::Value>),

    /// An IO stream of JSON data
    ///
    /// This takes over the data stream to allow JSON values to be sent directly to the command, and also allows for
    /// piping data from one command to another. IO streams can be used to supply data in arbitrary amounts to a command,
    /// and can also be used to repurpose a connection - for example, as an exclusive communication channel with another
    /// subprogram.
    ///
    /// IO streams are particularly useful as a form of RPC, making it possible to run a 
    IoStream(Box<dyn Send + FnOnce(BoxStream<'static, serde_json::Value>) -> BoxStream<'static, serde_json::Value>>),

    /// An interactive stream function (callback to take over the raw command data stream)
    ///
    /// This allows a command to take over the output stream, say to provide a TUI as part of an interactive session,
    /// or to provide binary data directly. Non-interactive command sessions may ignore this request and not allow
    /// direct access to their underlying stream: the general expectation is that the command stream is a terminal
    /// session that is being interacted with by a user.
    InteractiveStream(Box<dyn Send + FnOnce(BoxStream<'static, Vec<u8>>) -> BoxStream<'static, Vec<u8>>>),

    /// An error message, written as '!!! <error>'
    Error(String),    
}

///
/// `CommandResponseData` is like `CommandResponse` except that it takes a serializable data type: it can be used
/// in `with_json_command` to automatically serialize the response data for the command.
///
pub enum CommandResponseData<TResponseData>
where
    TResponseData: Serialize,
{
    /// A commentary message, written as '  <message>'
    Message(String),

    /// A JSON value, written out directly
    Data(TResponseData),

    /// A stream of values that can be outputted at any time, used for receiving monitored events
    /// A new stream is given a number in the initial response using a message of format '<<< <n>' (eg, '<<< 8')
    /// Events from that stream are displayed as '<<n> <json>', eg '<8 [ 1, 2, 3, 4 ]' - note that the JSON can
    /// spread across several lines. When the stream is closed, a '<EOS <n>' message is generated.
    BackgroundStream(BoxStream<'static, serde_json::Value>),

    /// An error message, written as '!!! <error>'
    Error(String),    
}

impl SceneMessage for CommandRequest {
    #[inline]
    fn message_type_name() -> String { "flo_scene_pipe::CommandRequest".into() }
}

impl SceneMessage for CommandResponse {
    #[inline]
    fn message_type_name() -> String { "flo_scene_pipe::CommandResponse".into() }
}

impl<TResponseData> From<TResponseData> for CommandResponseData<TResponseData>
where
    TResponseData: Serialize,
{
    fn from(data: TResponseData) -> Self {
        CommandResponseData::Data(data)
    }
}

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

impl<TResponseData> TryInto<CommandResponse> for CommandResponseData<TResponseData> 
where
    TResponseData: Serialize
{
    type Error = serde_json::Error;

    fn try_into(self) -> Result<CommandResponse, serde_json::Error> {
        match self {
            CommandResponseData::Data(data) => {
                let json_data = data.serialize(serde_json::value::Serializer)?;

                Ok(CommandResponse::Json(json_data))
            }

            CommandResponseData::Message(msg)               => Ok(CommandResponse::Message(msg)),
            CommandResponseData::BackgroundStream(stream)   => Ok(CommandResponse::BackgroundStream(stream)),
            CommandResponseData::Error(err)                 => Ok(CommandResponse::Error(err)),
        }
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
            CommandResponse::IoStream(_)            => write!(f, "IoStream(...)"),
            CommandResponse::InteractiveStream(_)   => write!(f, "InteractiveStream(...)"),
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
    pub async fn parse(command: &str) -> Result<CommandRequest, CommandParseError> {
        let mut parser      = Parser::new();
        let mut tokenizer   = Tokenizer::new(stream::iter(command.bytes()).ready_chunks(256));

        tokenizer.with_command_matchers();

        command_parse(&mut parser, &mut tokenizer).await?;

        Ok(parser.finish()?)
    }
}

impl Into<String> for VariableName {
    fn into(self) -> String {
        self.0
    }
}

impl<'a> Into<String> for &'a VariableName {
    fn into(self) -> String {
        self.0.clone()
    }
}

///
/// Reads data for the command program socket from an input stream
///
/// Often used with a socket, for example `start_internal_socket_program(&scene, socket_program, read_command_data, write_command_data)`
///
pub fn read_command_data(input: impl 'static + Send + Unpin + Stream<Item=Vec<u8>>) -> impl 'static + Send + Unpin + Stream<Item=CommandData> {
    input.map(|data| CommandData(data))
}

///
/// Converts command data to bytes ready to be sent to a socket stream
///
/// Often used with a socket, for example `start_internal_socket_program(&scene, socket_program, read_command_data, write_command_data)`
///
pub fn write_command_data(input: impl 'static + Send + Unpin + Stream<Item=CommandData>) -> BoxStream<'static, Vec<u8>> {
    input.map(|CommandData(data)| data).boxed()
}

///
/// Serialized forms of a JSON command response. Note that the streaming forms can't be supported in a simple serialized response, so
/// we just say they were present and convert them to errors on the other side
///
#[derive(Serialize, Deserialize)]
enum SerializedCommandResponse {
    Json(serde_json::Value),
    Message(String),

    BackgroundStreamNotSupported,
    IoStreamNotSupported,
    InteractiveStreamNotSupported,

    Error(String),    
}

impl Serialize for CommandResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        match self {
            CommandResponse::Json(json)             => SerializedCommandResponse::Json(json.clone()),
            CommandResponse::Message(msg)           => SerializedCommandResponse::Message(msg.clone()),
            CommandResponse::Error(err)             => SerializedCommandResponse::Error(err.clone()),

            CommandResponse::BackgroundStream(_)    => SerializedCommandResponse::BackgroundStreamNotSupported,
            CommandResponse::IoStream(_)            => SerializedCommandResponse::IoStreamNotSupported,
            CommandResponse::InteractiveStream(_)   => SerializedCommandResponse::InteractiveStreamNotSupported,
        }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CommandResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        match SerializedCommandResponse::deserialize(deserializer)? {
            SerializedCommandResponse::Json(json)   => Ok(CommandResponse::Json(json)),
            SerializedCommandResponse::Message(msg) => Ok(CommandResponse::Message(msg)),
            SerializedCommandResponse::Error(err)   => Ok(CommandResponse::Error(err)),

            SerializedCommandResponse::BackgroundStreamNotSupported     => Ok(CommandResponse::Error("Background stream not supported".into())),
            SerializedCommandResponse::IoStreamNotSupported             => Ok(CommandResponse::Error("I/O stream not supported".into())),
            SerializedCommandResponse::InteractiveStreamNotSupported    => Ok(CommandResponse::Error("Interactive stream not supported".into())),
        }
    }
}
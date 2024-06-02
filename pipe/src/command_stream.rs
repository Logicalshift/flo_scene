use flo_scene::*;

use futures::prelude::*;

use serde::{Deserialize, Serialize};
use serde_json;

///
/// A string value representing the name of a command sent to a stream
///
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CommandName(pub String);

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
pub struct Command {
    pub command:    CommandName,
    pub arguments:  Vec<CommandArgument>,
}

impl SceneMessage for Command { }

///
/// Reads an input stream containing commands in text form and outputs the command structures as they are matched
///
/// Commands are relatively simple, they have the structure `<name> <parameters>` where the name is an identifier (containing alphanumeric characters, 
/// alongside '_', '.' and ':'). Parameters are just JSON values, and commands are ended by a newline character that is outside of a JSON value.
///
pub fn parse_command_stream(input: impl 'static + Send + Unpin + Stream<Item=Vec<u8>>) -> impl 'static + Send + Unpin + Stream<Item=Command> {
    stream::empty()
}
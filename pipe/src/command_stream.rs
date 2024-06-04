use crate::parser::*;
use crate::parse_command::*;

use flo_scene::*;

use futures::prelude::*;

use serde::{Deserialize, Serialize};
use serde_json;
use flo_stream::{generator_stream};

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
pub enum Command {
    Command { command: CommandName, argument: serde_json::Value },
    Pipe    { from: Box<Command>, to: Box<Command> },
    Assign  { variable: VariableName, from: Box<Command> },
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

impl SceneMessage for Command { }

///
/// Reads an input stream containing commands in text form and outputs the command structures as they are matched
///
/// Commands are relatively simple, they have the structure `<name> <parameters>` where the name is an identifier (containing alphanumeric characters, 
/// alongside '_', '.' and ':'). Parameters are just JSON values, and commands are ended by a newline character that is outside of a JSON value.
///
pub fn parse_command_stream(input: impl 'static + Send + Unpin + Stream<Item=Vec<u8>>) -> impl 'static + Send + Unpin + Stream<Item=Result<Command, ()>> {
    generator_stream(move |yield_value| async move {
        let mut tokenizer   = Tokenizer::new(input);
        let mut parser      = Parser::new();

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

                    // TODO: parse until 
                }
            }
        }
    })
}
use flo_scene::*;

use serde_json;

///
/// A string value representing the name of a command sent to a stream
///
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandName(pub String);

///
/// An argument to a command sent to a stream
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandArgument {
    Json(serde_json::Value)
}

///
/// A command parsed from an input stream
///
/// Commands have the format `<CommandName> <Argument>*`, where the command name is an identifier and the arguments are JSON
/// values.
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub command:    CommandName,
    pub arguments:  Vec<CommandArgument>,
}

impl SceneMessage for Command { }

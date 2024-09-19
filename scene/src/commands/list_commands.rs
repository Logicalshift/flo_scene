use crate::scene_message::*;

use serde::*;

///
/// Description of a command as returned by ListCommand
///
#[derive(Clone, PartialEq, Eq, Debug)]
#[derive(Serialize, Deserialize)]
pub struct CommandDescription {
    /// The name of the command
    pub name: String,
}

///
/// As part of a response to a list commands request, this indicates the name of a command supported by the sender. This
/// is often used with a conversion into the response type of a command.
///
#[derive(Clone, PartialEq, Eq, Debug)]
#[derive(Serialize, Deserialize)]
pub struct ListCommandResponse(pub Vec<CommandDescription>);

impl SceneMessage for ListCommandResponse { }

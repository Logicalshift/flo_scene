use crate::scene_message::*;

#[cfg(feature="serde_support")] use serde::*;

///
/// As part of a response to a list commands request, this indicates the name of a command supported by the sender. This
/// is often used with a conversion into the response type of a command.
///
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature="serde_support", derive(Serialize, Deserialize))]
pub struct ListCommandResponse(pub String);

impl SceneMessage for ListCommandResponse { }

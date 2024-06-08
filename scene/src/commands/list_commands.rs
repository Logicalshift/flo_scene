use crate::scene_message::*;

///
/// As part of a response to a list commands request, this indicates the name of a command supported by the sender. This
/// is often used with a conversion into the response type of a command.
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ListCommandResponse(pub String);

impl SceneMessage for ListCommandResponse { }

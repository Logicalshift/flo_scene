use super::command_stream::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

///
/// A JSON command is a command that uses a JSON value as a request and returns a `CommandResponse` (which is usually a JSON value)
///
pub struct JsonCommand(RunCommand<serde_json::Value, CommandResponse>);

impl From<RunCommand<serde_json::Value, CommandResponse>> for JsonCommand {
    #[inline]
    fn from(cmd: RunCommand<serde_json::Value, CommandResponse>) -> Self {
        JsonCommand(cmd)
    }
}

impl Into<RunCommand<serde_json::Value, CommandResponse>> for JsonCommand {
    #[inline]
    fn into(self) -> RunCommand<serde_json::Value, CommandResponse> {
        self.0
    }
}

impl QueryRequest for JsonCommand {
    type ResponseData = CommandResponse;

    #[inline]
    fn with_new_target(self, new_target: StreamTarget) -> Self {
        JsonCommand(self.0.with_new_target(new_target))
    }
}

///
/// Starts a dispatcher that will forward `RunCommand<serde_json::Value, CommandResponse>` requests to the
/// program that can handle them.
///
pub fn start_json_command_dispatcher(scene: &Scene, program_id: SubProgramId) {
    scene.add_subprogram(program_id, command_dispatcher_subprogram::<serde_json::Value, CommandResponse>, 1)
}

impl SceneMessage for JsonCommand {

}

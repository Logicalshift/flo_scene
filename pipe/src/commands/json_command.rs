use super::command_stream::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use once_cell::sync::{Lazy};

/// The filter converts from JsonCommand to RunCommands so we can use the standard dispatcher without any other interposer
static      FILTER_CONVERT_JSON_COMMAND:    Lazy<FilterHandle>  = Lazy::new(|| FilterHandle::conversion_filter::<JsonCommand, RunCommand<serde_json::Value, CommandResponse>>());

/// The default JSON command dispatcher subprogram (which is also started automatically on sending a `JsonCommand`)
pub static  JSON_DISPATCHER_SUBPROGRAM:     StaticSubProgramId  = StaticSubProgramId::called("flo_scene_pipe::json_dispatcher");

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

impl JsonCommand {
    ///
    /// Creates a new 'run command' request. The command with the specified name will be run, and will send its response to the target.
    ///
    pub fn new(target: impl Into<StreamTarget>, name: impl Into<String>, parameter: impl Into<serde_json::Value>) -> Self {
        Self(RunCommand::new(target, name, parameter))
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
    fn default_target() -> StreamTarget {
        (*JSON_DISPATCHER_SUBPROGRAM).into()
    }

    fn initialise(scene: &Scene) {
        // Always run a JSON command dispatcher (this dispatches the 'run command' request)
        start_json_command_dispatcher(scene, *JSON_DISPATCHER_SUBPROGRAM);

        // JsonCommand requests can get converted when sent to the default dispatcher
        scene.connect_programs((), StreamTarget::Filtered(*FILTER_CONVERT_JSON_COMMAND, *JSON_DISPATCHER_SUBPROGRAM), StreamId::with_message_type::<JsonCommand>()).unwrap();
    }
}

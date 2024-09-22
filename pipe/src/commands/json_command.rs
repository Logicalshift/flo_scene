use super::command_stream::*;

use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;

use once_cell::sync::{Lazy};
use serde::*;

/// The filter converts from JsonCommand to RunCommands so we can use the standard dispatcher without any other interposer
static      FILTER_CONVERT_JSON_COMMAND:    Lazy<FilterHandle>  = Lazy::new(|| FilterHandle::conversion_filter::<JsonCommand, RunCommand<JsonParameter, CommandResponse>>());

/// The default JSON command dispatcher subprogram (which is also started automatically on sending a `JsonCommand`)
pub static  JSON_DISPATCHER_SUBPROGRAM:     StaticSubProgramId  = StaticSubProgramId::called("flo_scene_pipe::json_dispatcher");

///
/// Parameter to a JSON command
///
#[derive(Clone, PartialEq, Debug)]
#[derive(Serialize, Deserialize)]
pub struct JsonParameter {
    /// The value of the parameter
    pub value: serde_json::Value,

    /// The command processor that launched this command (should accept the `CommandProcessRequest` message). Can be None if the command was not launched by a 
    /// command processor
    pub processor: Option<SubProgramId>,
}

///
/// A JSON command is a command that uses a JSON value as a request and returns a `CommandResponse` (which is usually a JSON value)
///
#[derive(Serialize, Deserialize)]
pub struct JsonCommand(RunCommand<JsonParameter, CommandResponse>);

impl From<()> for JsonParameter {
    #[inline]
    fn from(_: ()) -> Self {
        JsonParameter {
            value:      serde_json::Value::Null,
            processor:  None
        }
    }
}

impl From<RunCommand<JsonParameter, CommandResponse>> for JsonCommand {
    #[inline]
    fn from(cmd: RunCommand<JsonParameter, CommandResponse>) -> Self {
        JsonCommand(cmd)
    }
}

impl Into<RunCommand<JsonParameter, CommandResponse>> for JsonCommand {
    #[inline]
    fn into(self) -> RunCommand<JsonParameter, CommandResponse> {
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
    /// If supplied, the `command_processor` can be used to query the environment the command is launched in using the `CommandProcessRequest` message.
    ///
    pub fn new(target: impl Into<StreamTarget>, name: impl Into<String>, parameter: impl Into<serde_json::Value>, command_processor: Option<SubProgramId>) -> Self {
        Self(RunCommand::new(target, name, JsonParameter { value: parameter.into(), processor: command_processor }))
    }
}

///
/// Starts a dispatcher that will forward `RunCommand<serde_json::Value, CommandResponse>` requests to the
/// program that can handle them.
///
pub fn start_json_command_dispatcher(scene: &Scene, program_id: SubProgramId) {
    scene.add_subprogram(program_id, command_dispatcher_subprogram::<JsonParameter, CommandResponse>, 1)
}

impl SceneMessage for JsonCommand {
    fn default_target() -> StreamTarget {
        (*JSON_DISPATCHER_SUBPROGRAM).into()
    }

    #[inline]
    fn message_type_name() -> String { "flo_scene_pipe::JsonCommand".into() }

    fn initialise(scene: &Scene) {
        // Always run a JSON command dispatcher (this dispatches the 'run command' request)
        start_json_command_dispatcher(scene, *JSON_DISPATCHER_SUBPROGRAM);

        // JsonCommand requests can get converted when sent to the default dispatcher
        scene.connect_programs((), StreamTarget::Filtered(*FILTER_CONVERT_JSON_COMMAND, *JSON_DISPATCHER_SUBPROGRAM), StreamId::with_message_type::<JsonCommand>()).unwrap();
    }
}

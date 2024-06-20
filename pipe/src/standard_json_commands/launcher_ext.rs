use super::connect::*;
use super::echo::*;
use super::help::*;
use super::list_connections::*;
use super::list_subprograms::*;
use super::query::*;
use super::send::*;
use super::subscribe::*;
use crate::commands::*;

use flo_scene::commands::*;

///
/// Extensions for the command launcher that add the 'standard' set of commands
///
pub trait StandardCommandsLauncherExt {
    ///
    /// Installs the standard JSON commands on this launcher
    ///
    fn with_standard_commands(self) -> Self;
}

impl StandardCommandsLauncherExt for CommandLauncher<serde_json::Value, CommandResponse> {
    fn with_standard_commands(self) -> Self {
        self
            .with_command("echo", command_echo)
            .with_json_command("connect", command_connect)
            .with_json_command("help", command_help)
            .with_json_command("list_connections", command_list_connections)
            .with_json_command("list_subprograms", command_list_subprograms)
            .with_json_command("query", command_query)
            .with_json_command("send", command_send)
            .with_json_command("subscribe", command_subscribe)
    }
}

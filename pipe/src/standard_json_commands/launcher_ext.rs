use super::echo::*;
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
    }
}

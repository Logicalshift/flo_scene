use super::{launcher_ext::*, ListConnectionsResponse, ListSubprogramsResponse};
use crate::commands::*;

use flo_scene::*;
use flo_scene::commands::*;

///
/// Extensions for a `Scene` that adds the standard set of JSON commands to a launcher
///
pub trait StandardCommandsSceneExt {
    ///
    /// Installs a launcher for the standard set of JSON commands
    ///
    fn with_standard_json_commands(self) -> Self;

    ///
    /// Creates a default scene with the standard JSON commands added
    ///
    fn default_with_json_commands() -> Self;
}

impl StandardCommandsSceneExt for Scene {
    fn with_standard_json_commands(self) -> Self {
        let launcher = CommandLauncher::json().with_standard_commands();

        self.add_subprogram(SubProgramId::new(), launcher.to_subprogram(), 0);

        self
    }

    fn default_with_json_commands() -> Self {
        let scene = Self::default()
            .with_standard_json_commands();

        scene.with_serializer(|| serde_json::value::Serializer)
            .with_serializable_type::<ListSubprogramsResponse>("flo_scene_pipe::ListSubprogramsResponse")
            .with_serializable_type::<ListConnectionsResponse>("flo_scene_pipe::ListConnectionsResponse");

        scene
    }
}

use super::control::*;
use crate::host::error::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

///
/// Context extension trait that provides convenience functions for interacting with the SceneControl
///
pub trait SceneControlExt {
    /// Adds a new subprogram to the scene
    fn add_subprogram<TProgramFn, TInputMessage, TFuture>(&self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Result<(), ConnectionError>;

    /// Adds a child of the current subprogram to the scene
    ///
    /// Child subprograms will stop when the running subprogram stops
    fn add_child_subprogram<TProgramFn, TInputMessage, TFuture>(&self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Result<(), ConnectionError>;

    /// Creates a connection between two subprograms
    fn connect_programs(&self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<(), ConnectionError>;

    /// Closes the input stream for a subprogram (which will shut it down)
    fn close_subprogram(&self, program: SubProgramId) -> Result<(), ConnectionError>;

    /// Adds a tag to the current subprogram
    fn tag(&self, tag: impl Into<SceneProgramTag>) -> Result<(), ConnectionError>;

    /// Adds a name for the current subprogram
    fn i_am(&self, name: impl Into<String>) { self.tag(SceneProgramTag::Name(name.into())).ok(); }

    /// Adds a task for the current subprogram
    fn my_task_is(&self, task: impl Into<String>) { self.tag(SceneProgramTag::Task(task.into())).ok(); }
}

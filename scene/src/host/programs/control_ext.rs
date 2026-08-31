use super::control::*;
use crate::host::error::*;
use crate::host::input_stream::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use futures::prelude::*;

///
/// Context extension trait that provides convenience functions for interacting with the SceneControl
///
pub trait SceneControlExt {
    /// Adds a new subprogram to the scene
    fn add_subprogram<TProgramFn, TInputMessage, TFuture>(&self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Result<(), ConnectionError>
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'static + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture;

    /// Adds a child of the current subprogram to the scene
    ///
    /// Child subprograms will stop when the running subprogram stops
    fn add_child_subprogram<TProgramFn, TInputMessage, TFuture>(&self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Result<(), ConnectionError>
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'static + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture;

    /// Creates a connection between two subprograms
    fn connect_programs(&self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<(), ConnectionError>;

    /// Closes the input stream for a subprogram (which will shut it down)
    fn close_subprogram(&self, program_id: SubProgramId) -> Result<(), ConnectionError>;

    /// Adds a tag to the current subprogram
    fn tag(&self, tag: impl Into<SceneProgramTag>) -> Result<(), ConnectionError>;

    /// Adds a name for the current subprogram
    fn i_am(&self, name: impl Into<String>) { self.tag(SceneProgramTag::Name(name.into().into())).ok(); }

    /// Adds a task for the current subprogram
    fn my_task_is(&self, task: impl Into<String>) { self.tag(SceneProgramTag::Task(task.into().into())).ok(); }
}

impl SceneControlExt for SceneContext {
    fn add_subprogram<TProgramFn, TInputMessage, TFuture>(&self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Result<(), ConnectionError> 
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'static + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture
    {
        let mut queue = self.send(())?;

        self.run_in_background(async move {
            queue.send(SceneControl::start_program(program_id, program, max_input_waiting)).await.ok();
        });

        Ok(())
    }

    fn add_child_subprogram<TProgramFn, TInputMessage, TFuture>(&self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Result<(), ConnectionError>
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'static + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture
    {
        let Some(parent_program_id) = self.current_program_id() else { return Err(ConnectionError::SubProgramNotRunning); };
        let mut queue               = self.send(())?;

        self.run_in_background(async move {
            queue.send(SceneControl::start_child_program(program_id, parent_program_id, program, max_input_waiting)).await.ok();
        });

        Ok(())
    }

    fn connect_programs(&self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<(), ConnectionError> {
        let source      = source.into();
        let target      = target.into();
        let stream      = stream.into();
        let mut queue   = self.send(())?;

        self.run_in_background(async move {
            queue.send(SceneControl::connect(source, target, stream)).await.ok();
        });

        Ok(())
    }

    fn close_subprogram(&self, program_id: SubProgramId) -> Result<(), ConnectionError> {
        let mut queue = self.send(())?;

        self.run_in_background(async move {
            queue.send(SceneControl::Close(program_id)).await.ok();
        });

        Ok(())
    }

    fn tag(&self, tag: impl Into<SceneProgramTag>) -> Result<(), ConnectionError> {
        let Some(our_program_id)    = self.current_program_id() else { return Err(ConnectionError::SubProgramNotRunning); };
        let tag                     = tag.into();
        let mut queue               = self.send(())?;

        self.run_in_background(async move {
            queue.send(SceneControl::Tag(our_program_id, tag)).await.ok();
        });

        Ok(())
    }
}

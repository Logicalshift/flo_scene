use crate::input_stream::*;
use crate::scene_context::*;
use crate::scene_core::*;
use crate::stream_id::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::channel::oneshot;

use std::sync::*;

///
/// A scene represents a set of running co-programs, creating a larger self-contained piece of
/// software out of a set of smaller pieces of software that communicate via streams.
///
pub struct Scene {
    core: Arc<Mutex<SceneCore>>,
}

impl Default for Scene {
    fn default() -> Self {
        // Create an empty scene
        let scene = Scene::empty();

        // Populate with the default programs
        // TODO: 'main' program for starting/stopping other programs and wiring streams

        scene
    }
}

impl Scene {
    ///
    /// Creates an empty scene (this has no control program so it won't start or connect any programs by default)
    ///
    pub fn empty() -> Self {
        Scene {
            core: Arc::new(Mutex::new(SceneCore::new()))
        }
    }

    ///
    /// Adds a subprogram to run in this scene
    ///
    pub fn add_subprogram<TProgramFn, TInputMessage, TFuture>(&self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize)
    where
        TFuture:        Send + Sync + Future<Output=()>,
        TInputMessage:  'static + Unpin + Send + Sync,
        TProgramFn:     'static + Send + Sync + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture,
    {
        // Create the context and input stream for the program
        let input_stream    = InputStream::new(program_id.clone(), max_input_waiting);
        let input_core      = input_stream.core();

        // Create the future that will be used to run the future
        let (send_context, recv_context) = oneshot::channel();
        let run_program = async move {
            if let Ok(context) = recv_context.await {
                program(input_stream, context).await;
            }
        };

        // Start the program running
        let (subprogram, waker) = {
            let mut core = self.core.lock().unwrap();
            core.start_subprogram(program_id, run_program, input_core)
        };

        // Create the scene context, and send it to the subprogram
        let context = SceneContext::new(&self.core, &subprogram);
        send_context.send(context).ok();

        // Wake the scene up
        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Specifies that an output of `source` (identified by the StreamId) should be connected to the input of `target`
    ///
    pub fn connect_programs(&self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<(), ()> {
        todo!()
    }

    ///
    /// Returns a future that will run any waiting programs on the current thread
    ///
    pub fn run_scene(&self) -> impl Future<Output=()> {
        run_core(&self.core)
    }
}

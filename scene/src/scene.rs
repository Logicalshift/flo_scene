use crate::{SceneContext, SubProgramId, StreamId, StreamSource, StreamTarget};
use crate::scene_core::*;

use futures::prelude::*;
use futures::channel::mpsc;

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
    pub fn add_subprogram<TProgramFn, TInputMessage, TFuture>(&self, program_id: SubProgramId, program: TProgramFn)
    where
        TFuture:        Send + Sync + Future<Output=()>,
        TInputMessage:  Send + Sync,
        TProgramFn:     'static + Fn(mpsc::Receiver<TInputMessage>, &SceneContext) -> TFuture,
    {
        todo!()
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

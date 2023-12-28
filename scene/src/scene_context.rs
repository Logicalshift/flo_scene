use crate::scene_core::*;

use futures::prelude::*;
use futures::channel::mpsc;

use std::sync::*;

///
/// The scene context is a per-subprogram way to access output streams
///
/// The context is passed to the program when it starts, and can also be retrieved from any code executing as part of that subprogram.
///
#[derive(Clone)]
pub struct SceneContext {
    /// The core of the running scene (if it still exists)
    scene_core: Weak<Mutex<SceneCore>>,

    /// The program the 
    program_core: Weak<Mutex<SubProgramCore>>,
}

impl SceneContext {
    pub (crate) fn new(scene_core: &Arc<Mutex<SceneCore>>, program_core: &Arc<Mutex<SubProgramCore>>) -> Self {
        SceneContext {
            scene_core:     Arc::downgrade(scene_core),
            program_core:   Arc::downgrade(program_core),
        }
    }

    ///
    /// Retrieves a stream for sending messages of the specified type
    ///
    /// If no receiver is attached to this stream type for this program, the 
    ///
    pub fn send<TMessageType>(&self) -> impl Sink<TMessageType>
    where
        TMessageType: 'static + Send + Sync,
    {
        todo!();

        let (send, _recv ) = mpsc::channel(1);
        send
    }
}

use crate::output_sink::*;
use crate::scene_core::*;
use crate::stream_id::*;
use crate::stream_target::*;

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

    /// The program that's running in this context
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
    /// The target can be used to define the default destination for the stream. If the target is a specific program, that program should
    /// have an input type that matches the message type. If the target is `None` or `Any`, the stream can be connected by the scene (by the
    /// `connect_programs()` request), so the exact target does not need to be known.
    ///
    /// The `None` target will discard any messages received while the stream is disconnected, but the `Any` target will block until something
    /// connects the stream. Streams with a specified target will connect to that target immediately.
    ///
    pub fn send<TMessageType>(&self, target: impl Into<StreamTarget>) -> Result<impl Sink<TMessageType>, ()>
    where
        TMessageType: 'static + Unpin + Send + Sync,
    {
        if let (Some(scene_core), Some(program_core)) = (self.scene_core.upgrade(), self.program_core.upgrade()) {
            // Convert the target to a stream ID. If we need to create the sink target, we can create it in 'wait' or 'discard' mode
            let target                          = target.into();
            let (stream_id, discard_by_default) = match &target {
                StreamTarget::None              => (StreamId::with_message_type::<TMessageType>(), true),
                StreamTarget::Any               => (StreamId::with_message_type::<TMessageType>(), false),
                StreamTarget::Program(prog_id)  => (StreamId::for_target::<TMessageType>(prog_id.clone()), false),
            };

            // Try to re-use an existing target
            let (existing_target, program_id) = {
                let program_core = program_core.lock().unwrap();
                (program_core.output_target(&stream_id), program_core.program_id().clone())
            };

            if let Some(existing_target) = existing_target {
                // This program has previously created a stream for this target (or had a stream connected by the scene)
                let sink = OutputSink::attach(program_id, existing_target);

                Ok(sink)
            } else {
                // Create a new target
                let mut scene_core  = scene_core.lock().unwrap();
                let new_target  = scene_core.sink_for_target(&program_id, target);

                if let Some(new_target) = new_target {
                    // The scene core could provide a sink target for this stream, which we'll set in the program core
                    // Locking both so the scene's target can't change before we're done
                    let mut program_core = program_core.lock().unwrap();
                    Ok(match program_core.try_create_output_target(&stream_id, new_target) {
                        Ok(new_target)  => OutputSink::attach(program_id, new_target),
                        Err(old_target) => OutputSink::attach(program_id, old_target),
                    })
                } else {
                    // The scene core has no sink target, so we'll create a generic one that can be updated later
                    let new_target = if discard_by_default { OutputSinkTarget::Discard } else { OutputSinkTarget::Disconnected };
                    let new_target = Arc::new(Mutex::new(new_target));

                    let mut program_core = program_core.lock().unwrap();
                    Ok(match program_core.try_create_output_target(&stream_id, new_target) {
                        Ok(new_target)  => OutputSink::attach(program_id, new_target),
                        Err(old_target) => OutputSink::attach(program_id, old_target),
                    })
                }
            }
        } else {
            // TODO: Return an error (scene or program has finished)
            todo!()
        }
    }
}

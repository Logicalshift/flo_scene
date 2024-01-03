use crate::error::*;
use crate::output_sink::*;
use crate::scene_core::*;
use crate::stream_id::*;
use crate::stream_target::*;

use futures::prelude::*;

use std::cell::*;
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
    pub fn send<TMessageType>(&self, target: impl Into<StreamTarget>) -> Result<impl Sink<TMessageType, Error=SceneSendError>, ConnectionError>
    where
        TMessageType: 'static + Unpin + Send + Sync,
    {
        if let (Some(scene_core), Some(program_core)) = (self.scene_core.upgrade(), self.program_core.upgrade()) {
            // Convert the target to a stream ID. If we need to create the sink target, we can create it in 'wait' or 'discard' mode
            let target      = target.into();
            let stream_id   = match &target {
                StreamTarget::None                      => StreamId::with_message_type::<TMessageType>(),
                StreamTarget::Any                       => StreamId::with_message_type::<TMessageType>(),
                StreamTarget::Program(prog_id)          => StreamId::for_target::<TMessageType>(prog_id.clone()),
                StreamTarget::Filtered(filter, prog_id) => filter.target_stream_id(*prog_id)?,
            };

            // Try to re-use an existing target
            let (existing_core, program_id) = {
                let program_core = program_core.lock().unwrap();
                (program_core.output_core(&stream_id), program_core.program_id().clone())
            };

            if let Some(existing_core) = existing_core {
                // This program has previously created a stream for this target (or had a stream connected by the scene)
                let sink = OutputSink::attach(program_id, existing_core);

                Ok(sink)
            } else {
                // Fetch the target from the core (possibly creating a new one)
                let new_target  = SceneCore::sink_for_target(&scene_core, &program_id, target)?;

                // The scene core could provide a sink target for this stream, which we'll set in the program core
                // Locking both so the scene's target can't change before we're done
                let mut program_core = program_core.lock().unwrap();
                Ok(match program_core.try_create_output_target(&stream_id, new_target) {
                    Ok(new_target)  => OutputSink::attach(program_id, new_target),
                    Err(old_target) => OutputSink::attach(program_id, old_target),
                })
            }
        } else {
            // Scene or program has been stopped
            Err(ConnectionError::TargetNotAvailable)
        }
    }

    ///
    /// Sends a single message to the default output of that type
    ///
    pub async fn send_message<TMessageType>(&self, message: TMessageType) -> Result<(), ConnectionError> 
    where
        TMessageType: 'static + Unpin + Send + Sync,
    {
        let mut stream = self.send::<TMessageType>(())?;

        stream.send(message).await?;

        Ok(())
    }

    ///
    /// Retrieves the scene core for this context
    ///
    pub (crate) fn scene_core(&self) -> Weak<Mutex<SceneCore>> {
        self.scene_core.clone()
    }
}

thread_local! {
    static ACTIVE_CONTEXT: RefCell<Option<SceneContext>> = RefCell::new(None);
}

struct OldContext(Option<SceneContext>);

impl Drop for OldContext {
    fn drop(&mut self) {
        ACTIVE_CONTEXT.with(|active_context| *active_context.borrow_mut() = self.0.take());
    }
}

///
/// Performs an action with the specified context set as the thread context
///
pub fn with_scene_context<TReturnType>(context: &SceneContext, action: impl FnOnce() -> TReturnType) -> TReturnType {
    use std::mem;

    // Update the active context and create an old context
    let old_context = ACTIVE_CONTEXT.with(|active_context| {
        let old_context                 = OldContext(active_context.take());
        *active_context.borrow_mut()    = Some(context.clone());

        old_context
    });

    // Peform the action with the context set
    let result = action();

    // Finished with the old context now
    mem::drop(old_context);

    result
}

///
/// Returns the scene context set for the current thread
///
/// The scene context is automatically set while subprograms are being polled, and can also be manually set for
/// the duration of a function using `with_scene_context()`
///
pub fn scene_context() -> Option<SceneContext> {
    ACTIVE_CONTEXT.with(|active_context| active_context.borrow().clone())
}

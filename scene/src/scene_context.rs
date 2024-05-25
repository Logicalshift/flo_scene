use crate::error::*;
use crate::output_sink::*;
use crate::programs::*;
use crate::scene_core::*;
use crate::scene_message::*;
use crate::stream_id::*;
use crate::stream_target::*;
use crate::subprogram_core::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

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
    /// Returns the currently active subprogram, if there is one
    ///
    /// This will return 'None' if the scene that the program was running in is terminated but the
    /// task is still running, so this is a very rare occurrence. 
    ///
    pub fn current_program_id(&self) -> Option<SubProgramId> {
        let program_core    = self.program_core.upgrade()?;
        let program_id      = *program_core.lock().unwrap().program_id();

        Some(program_id)
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
    pub fn send<TMessageType>(&self, target: impl Into<StreamTarget>) -> Result<OutputSink<TMessageType>, ConnectionError>
    where
        TMessageType: 'static + SceneMessage,
    {
        use std::mem;

        if let (Some(scene_core), Some(program_core)) = (self.scene_core.upgrade(), self.program_core.upgrade()) {
            // Convert the target to a stream ID. If we need to create the sink target, we can create it in 'wait' or 'discard' mode
            let target      = target.into();
            let stream_id   = match &target {
                StreamTarget::None                      => StreamId::with_message_type::<TMessageType>(),
                StreamTarget::Any                       => StreamId::with_message_type::<TMessageType>(),
                StreamTarget::Program(prog_id)          => StreamId::with_message_type::<TMessageType>().for_target(*prog_id),
                StreamTarget::Filtered(filter, prog_id) => filter.target_stream_id(*prog_id)?,
            };

            // Try to re-use an existing target
            let (existing_core, program_id) = {
                let program_core = program_core.lock().unwrap();
                (program_core.output_core(&stream_id), *program_core.program_id())
            };

            if let Some(existing_core) = existing_core {
                // This program has previously created a stream for this target (or had a stream connected by the scene)
                let sink = OutputSink::attach(program_id, existing_core, &scene_core);

                Ok(sink)
            } else {
                // Fetch the target from the core (possibly creating a new one)
                let new_target  = SceneCore::sink_for_target(&scene_core, &program_id, target)?;

                // The scene core could provide a sink target for this stream, which we'll set in the program core
                // Locking both so the scene's target can't change before we're done
                let new_or_old_target = program_core.lock().unwrap().try_create_output_target(&stream_id, new_target);

                match new_or_old_target {
                    Ok(new_target) => {
                        // Clean out any stale connections
                        let stale_sinks = program_core.lock().unwrap().release_stale_output_sinks();
                        mem::drop(stale_sinks);

                        // Report the new connection
                        let target_program  = OutputSinkCore::target_program_id(&new_target);
                        let update          = if let Some(target_program) = target_program {
                            SceneUpdate::Connected(program_id, target_program, stream_id)
                        } else {
                            SceneUpdate::Disconnected(program_id, stream_id)
                        };

                        SceneCore::send_scene_updates(&scene_core, vec![update]);

                        // Attach the new target to an output sink
                        Ok(OutputSink::attach(program_id, new_target, &scene_core))
                    },

                    Err(old_target) => {
                        // Just re-use the old target
                        Ok(OutputSink::attach(program_id, old_target, &scene_core))
                    }
                }
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
        TMessageType: 'static + SceneMessage,
    {
        let mut stream = self.send::<TMessageType>(())?;

        stream.send(message).await?;

        Ok(())
    }

    ///
    /// Retrieves a stream for sending replies to the last message received by the current subprogram
    ///
    /// The usual way to design a message that generates some results is to supply a target subprogram with the requests
    /// rather than replying to the original message source. This may seem counterintuitive if you're used to a more
    /// 'function-oriented' approach - think in terms of how output is passed on when writing a shell script rather than
    /// the idea of a 'result' being returned to a sender.
    ///
    /// Replying is more useful for status updates and things like logging, where some feedback to the original sender
    /// might be useful (eg, a sender might be able to see what log messages are being generated for their requests)
    ///
    pub fn reply<TMessageType>(&self) -> Result<OutputSink<TMessageType>, ConnectionError>
    where
        TMessageType: 'static + SceneMessage,
    {
        if let Some(program_core) = self.program_core.upgrade() {
            // Retrieve the input core via the program ID
            let last_message_source = program_core.lock().unwrap().last_message_source;

            if let Some(last_message_source) = last_message_source {
                self.send(last_message_source)
            } else {
                // Target input stream is no longer available
                Err(ConnectionError::TargetNotInScene)
            }
        } else {
            // Scene or program is no longer running
            Err(ConnectionError::TargetNotInScene)
        }
    }

    ///
    /// Replies to the sender of the last message sent to this subprogram with another message
    ///
    /// The usual way to design a message that generates some results is to supply a target subprogram with the requests
    /// rather than replying to the original message source. This may seem counterintuitive if you're used to a more
    /// 'function-oriented' approach - think in terms of how output is passed on when writing a shell script rather than
    /// the idea of a 'result' being returned to a sender.
    ///
    /// Replying is more useful for status updates and things like logging, where some feedback to the original sender
    /// might be useful (eg, a sender might be able to see what log messages are being generated for their requests)
    ///
    pub async fn reply_with<TMessageType>(&self, message: TMessageType) -> Result<(), ConnectionError>
    where
        TMessageType: 'static + SceneMessage,
    {
        let mut stream = self.reply::<TMessageType>()?;

        stream.send(message).await?;

        Ok(())
    }

    ///
    /// Retrieves the scene core for this context
    ///
    pub (crate) fn scene_core(&self) -> Weak<Mutex<SceneCore>> {
        self.scene_core.clone()
    }

    ///
    /// A task processes an input stream and returns its output as a captured output stream
    ///
    /// Tasks are considered part of the subprogram that spawns them, and are useful as a way to get feedback from a query or run some background processing.
    ///
    /// The difference between a task and a subprogram is that a subprogram will accept input from multiple sources, while a task will only accept input from the supplied
    /// stream. A task should also process its input and then stop, where a subprogram generally runs indefinitely.
    ///
    /// The default stream for the specified `TOutput` type is captured from the spawned task and returned to the subprogram that spawned it, which makes it possible
    /// for the spawning subprogram to receive feedback from its subtasks.
    ///
    pub fn spawn_task<TInput, TOutput, TFuture>(task: impl Fn(BoxStream<'static, TInput>, SceneContext) -> TFuture, input: impl 'static + Send + Stream<Item=TInput>) -> Result<impl 'static + Send + Stream<Item=TOutput>, ConnectionError>
    where
        TInput:     'static + Send,
        TOutput:    'static + SceneMessage,
        TFuture:    'static + Send + Future<Output = ()>,
    {
        todo!();

        Ok(stream::empty())
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

use crate::command_trait::*;
use crate::error::*;
use crate::input_stream;
use crate::input_stream::*;
use crate::output_sink::*;
use crate::programs::*;
use crate::scene_core::*;
use crate::scene_message::*;
use crate::stream_id::*;
use crate::stream_target::*;
use crate::subprogram_core::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::channel::mpsc;

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
    /// Spawns a command to run in this scene, returning the command's standard output
    ///
    pub fn spawn_command<TCommand>(&self, command: TCommand, input: impl 'static + Send + Stream<Item=TCommand::Input>) -> Result<impl 'static + Stream<Item=TCommand::Output>, ConnectionError>
    where
        TCommand: 'static + Command,
    {
        if let (Some(scene_core), Some(program_core)) = (self.scene_core.upgrade(), self.program_core.upgrade()) {
            use std::mem;

            // Get the ID for this task
            let our_program_id  = program_core.lock().unwrap().id;
            let task_program_id = program_core.lock().unwrap().new_task_id();

            // The task has an input stream that is immediately closed (can't receive any input from elsewhere in the program)
            let closed_input_stream = InputStream::<()>::new(task_program_id, &scene_core, 0);
            let closed_input_core   = closed_input_stream.core();
            mem::drop(closed_input_stream);

            // The output of the command needs to be connected to an input stream (which is not the input of a subprogram in this case) 
            let command_result      = InputStream::new(our_program_id, &scene_core, 4);
            let command_result_core = command_result.core();

            // We need to receive the context after the subprogram has been added to the core
            let (send_context, recv_context)    = oneshot::channel::<SceneContext>();
            let command_result_core             = Arc::downgrade(&command_result_core);

            let run_program = async move {
                if let Ok(scene_context) = recv_context.await {
                    command.run(input, scene_context).await;

                    // Close the result stream once the command finishes running
                    if let Some(command_result_core) = command_result_core.upgrade() {
                        let waker = command_result_core.lock().unwrap().close();
                        if let Some(waker) = waker {
                            waker.wake();
                        }
                    }
                }
            };

            // Use the run_program future to spawn a new task in the scene
            let subtask = SceneCore::start_subprogram(&scene_core, task_program_id, run_program, closed_input_core);

            // Before allowing the program to proceed, share the subtask ID counter
            let id_counter = program_core.lock().unwrap().next_command_sequence.clone();
            subtask.lock().unwrap().next_command_sequence = id_counter;

            // Send the context to the waiting program
            let subtask_context = SceneContext::new(&scene_core, &subtask);
            send_context.send(subtask_context.clone()).ok();

            // Specify that the output for the standard stream is connected to 'Any' by default
            // (There's a bit of fragility over the output stream here, if it gets reconnected it will stop sending to us)
            SceneCore::connect_programs(&scene_core, task_program_id.into(), StreamTarget::Any, StreamId::with_message_type::<TCommand::Output>()).unwrap();

            // Create a stream from the command output stream (this is an extra input stream for the target program)
            let mut target_output_sink  = subtask_context.send::<TCommand::Output>(())?;
            let command_result_core     = command_result.core();

            target_output_sink.attach_to_core(&command_result_core);

            Ok(command_result)
        } else {
            // The core or the program is not running any more
            Err(ConnectionError::SubProgramNotRunning)
        }
    }

    ///
    /// Spawns a command that reads the response from a query to a target
    ///
    /// The query should be created with `StreamTarget::None` (any other target will also work, but because the query response will not be sent to the 
    /// specified target, 'None' makes this more clear)
    ///
    /// Some care should be taken when deciding to use a query command: this provides a more direct feedback mechanism from a target, but it is also
    /// somewhat equivalent to a function call, which may be a better mechanism. Normally, `flo_scene` subprograms send messages forward: so a typical
    /// design for a data storage mechanism would be to have a message that asks to retrieve the data and then sends it on to another subprogram. A query
    /// command could do the same thing but return the data to the sender instead, but so could a simple function call.
    ///
    /// A query command is a more heavyweight approach than a function call, so a positive reason for using one is needed.
    ///
    /// Sending a message instead of making a function call has some unique properties, however. For the database example, a function call requires more
    /// concrete knowledge of the database and a hard dependency: the query message on the other hand requires neither any knowledge of the actual database
    /// or any kind of dependency: both of these can be set up entirely independently. Queries can also be used to forward the response elsewhere. A 
    /// `query_target` of `StreamTarget::Any` is a good sign that this API is being used appropriately.
    ///
    pub fn spawn_query<TCommand>(&self, command: TCommand, query: impl 'static + QueryRequest<ResponseData=TCommand::Input>, query_target: impl Into<StreamTarget>) -> Result<impl 'static + Stream<Item=TCommand::Output>, ConnectionError>
    where
        TCommand: 'static + Command,
    {
        // TODO: this is very similar to spawn_command, might be more easy to maintain if some common core is extracted from both messages (the different handling of the input stream makes it tricky to find something natural, though)

        if let (Some(scene_core), Some(program_core)) = (self.scene_core.upgrade(), self.program_core.upgrade()) {
            use std::mem;

            // Get the ID for this task
            let our_program_id  = program_core.lock().unwrap().id;
            let task_program_id = program_core.lock().unwrap().new_task_id();

            // Connect to the target
            let mut target_connection = self.send(query_target)?;

            // The task has an input stream that is immediately closed (can't receive any input from elsewhere in the program)
            let response_input_stream = InputStream::<QueryResponse<TCommand::Input>>::new(task_program_id, &scene_core, 0);
            let response_input_core   = response_input_stream.core();

            // We generate a 
            let command_result      = InputStream::new(our_program_id, &scene_core, 4);
            let command_result_core = command_result.core();

            // We need to receive the context after the subprogram has been added to the core
            let (send_context, recv_context)    = oneshot::channel::<SceneContext>();
            let command_result_core             = Arc::downgrade(&command_result_core);

            let run_program = async move {
                if let Ok(scene_context) = recv_context.await {
                    // Send the query
                    let mut response_input_stream = response_input_stream;
                    if let Ok(()) = target_connection.send(query.with_new_target(task_program_id.into())).await {
                        // Wait for the response
                        if let Some(response) = response_input_stream.next().await {
                            // Refuse any further input
                            mem::drop(response_input_stream);

                            // Run the command with the response to the query
                            command.run(response, scene_context).await;

                            // Close the result stream once the command finishes running
                            if let Some(command_result_core) = command_result_core.upgrade() {
                                let waker = command_result_core.lock().unwrap().close();
                                if let Some(waker) = waker {
                                    waker.wake();
                                }
                            }
                        } else {
                            // Could not receive the response (TODO)
                        }
                    } else {
                        // Could not send the query (TODO: maybe make the input a TryStream?)
                    }
                }
            };

            // Use the run_program future to spawn a new task in the scene
            let subtask = SceneCore::start_subprogram(&scene_core, task_program_id, run_program, response_input_core);

            // Before allowing the program to proceed, share the subtask ID counter
            let id_counter = program_core.lock().unwrap().next_command_sequence.clone();
            subtask.lock().unwrap().next_command_sequence = id_counter;

            // Send the context to the waiting program
            let subtask_context = SceneContext::new(&scene_core, &subtask);
            send_context.send(subtask_context.clone()).ok();

            // Specify that the output for the standard stream is connected to 'Any' by default
            // (There's a bit of fragility over the output stream here, if it gets reconnected it will stop sending to us)
            SceneCore::connect_programs(&scene_core, task_program_id.into(), StreamTarget::Any, StreamId::with_message_type::<TCommand::Output>()).unwrap();

            // Create a stream from the command output stream (this is an extra input stream for the target program)
            let mut target_output_sink  = subtask_context.send::<TCommand::Output>(())?;
            let command_result_core     = command_result.core();

            target_output_sink.attach_to_core(&command_result_core);

            Ok(command_result)
        } else {
            // The core or the program is not running any more
            Err(ConnectionError::SubProgramNotRunning)
        }
    }

    ///
    /// Waits for the scene to become idle
    ///
    /// Subprograms that are waiting for the scene to become idle will produce errors if their input queue becomes full instead of blocking.
    ///
    /// The scene is idle if all the subprograms have 0 messages waiting and are ready to receive a new message, or are waiting for the scene to become
    /// idle. Idle notifications may be suppressed with the `IdleRequest::SuppressNotifications` request (in which case the scene is not considered
    /// idle until the corresponding `ResumeNotifications` request is made)
    ///
    /// When waiting for a scene to become idle, a subprogram may increase its maximum queue length. This is because messages waiting to be sent cannot
    /// block any more in this state, so will error out. So a program that needs to wait for the scene to become idle will typically need to raise its
    /// queue length high enough to accomodate any message that might be sent to it during this time (to avoid blocking the very condition it's waiting
    /// for).
    ///
    pub async fn wait_for_idle(&self, max_idle_queue_len: usize) {
        use std::mem;

        if let (Some(scene_core), Some(program_core)) = (self.scene_core.upgrade(), self.program_core.upgrade()) {
            // Get the input stream ID from the program core
            let (program_id, stream_id) = {
                let program_core = program_core.lock().unwrap();

                (*program_core.program_id(), program_core.input_stream_id())
            };

            // Fetch the input stream core for the current program
            let input_stream_core = scene_core.lock().unwrap().get_input_stream_core(program_id);
            let input_stream_core = if let Some(input_stream_core) = input_stream_core { input_stream_core } else { return; };

            // Say we're waiting for idle (so input can accrue without blocking the idle message)
            let we_are_waiting = stream_id.waiting_for_idle(&input_stream_core, max_idle_queue_len);

            if let Ok(we_are_waiting) = we_are_waiting {
                // Wait for the scene to become idle
                let (send, recv) = mpsc::channel(1);
                SceneCore::send_idle_notifications_to(&scene_core, send);
                SceneCore::notify_on_next_idle(&scene_core);

                let mut when_idle = recv;
                when_idle.next().await;

                // No longer waiting
                mem::drop(we_are_waiting);
            }
        }
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

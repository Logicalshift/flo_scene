use crate::host::connect_result::*;
use crate::host::input_stream::*;
use crate::host::output_sink::*;
use crate::host::scene_context::*;
use crate::host::scene_core::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;
use crate::host::error::*;
use crate::host::programs::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::future::{poll_fn};
use futures::{pin_mut};

use std::io::{stdin, stdout, stderr, BufReader};
use std::sync::*;
use std::collections::{HashSet};

///
/// A scene represents a set of running co-programs, creating a larger self-contained piece of
/// software out of a set of smaller pieces of software that communicate via streams.
///
#[derive(Clone)]
pub struct Scene {
    core: Arc<Mutex<SceneCore>>,
}

impl Default for Scene {
    fn default() -> Self {
        Scene::with_standard_programs([
            *SCENE_CONTROL_PROGRAM,
            *OUTSIDE_SCENE_PROGRAM,
            *STDIN_PROGRAM,
            *STDOUT_PROGRAM,
            *STDERR_PROGRAM,
            *IDLE_NOTIFICATION_PROGRAM,
            *TIMER_PROGRAM,
        ])
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
    /// Creates a new scene with a set of programs from the default set started
    ///
    /// For example, calling this as `Scene::with_standard_programs([*SCENE_CONTROL_PROGRAM])` will create a scene with only
    /// the standard scene control program running.
    ///
    pub fn with_standard_programs(programs: impl IntoIterator<Item=SubProgramId>) -> Self {
        let scene       = Self::empty();
        let programs    = programs.into_iter().collect::<HashSet<_>>();

        if programs.contains(&*SCENE_CONTROL_PROGRAM) {
            let control_updates = SceneCore::send_updates_to_stream(&scene.core, *SCENE_CONTROL_PROGRAM);

            scene.add_subprogram(*SCENE_CONTROL_PROGRAM, move |input, context| SceneControl::scene_control_program(input, context, control_updates), 0);
            scene.connect_programs((), *SCENE_CONTROL_PROGRAM, StreamId::with_message_type::<Subscribe<SceneUpdate>>()).unwrap();
            scene.connect_programs((), *SCENE_CONTROL_PROGRAM, StreamId::with_message_type::<Query<SceneUpdate>>()).unwrap();
        }
        if programs.contains(&*OUTSIDE_SCENE_PROGRAM)       { scene.add_subprogram(*OUTSIDE_SCENE_PROGRAM, outside_scene_program, 0); }

        if programs.contains(&*STDIN_PROGRAM)               { scene.add_subprogram(*STDIN_PROGRAM, |input, context| text_input_subprogram(BufReader::new(stdin()), input, context), 0); }
        if programs.contains(&*STDOUT_PROGRAM)              { scene.add_subprogram(*STDOUT_PROGRAM, |input, context| text_io_subprogram(stdout(), input, context), 0); }
        if programs.contains(&*STDERR_PROGRAM)              { scene.add_subprogram(*STDERR_PROGRAM, |input, context| text_io_subprogram(stderr(), input, context), 0); }
        if programs.contains(&*IDLE_NOTIFICATION_PROGRAM)   { scene.add_subprogram(*IDLE_NOTIFICATION_PROGRAM, idle_subprogram, 20); }
        if programs.contains(&*TIMER_PROGRAM)               { scene.add_subprogram(*TIMER_PROGRAM, timer_subprogram, 0); }

        scene
    }

    ///
    /// Creates a duplicate scene object
    ///
    pub (crate) fn with_core(core: &Arc<Mutex<SceneCore>>) -> Self {
        Scene {
            core: core.clone()
        }
    }

    ///
    /// Gets a reference to the core of this scene
    ///
    #[inline]
    pub (crate) fn core(&self) -> &Arc<Mutex<SceneCore>> {
        &self.core
    }

    ///
    /// Adds a subprogram to run in this scene
    ///
    pub fn add_subprogram<'a, TProgramFn, TInputMessage, TFuture>(&'a self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize)
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'a + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture,
    {
        // Create the context and input stream for the program
        let input_stream    = InputStream::new(program_id, &self.core, max_input_waiting);
        let input_core      = input_stream.core();

        // Create the future that will be used to run the future
        let (send_context, recv_context) = oneshot::channel::<(TFuture, SceneContext)>();
        let run_program = async move {
            if let Ok((program, scene_context)) = recv_context.await {
                // Start the program running
                pin_mut!(program);

                // Poll the program with the scene context set
                poll_fn(|context| {
                    with_scene_context(&scene_context, || {
                        program.as_mut().poll(context)
                    })
                }).await;
            }
        };

        // Start the program running
        let subprogram = SceneCore::start_subprogram(&self.core, program_id, run_program, input_core);

        // Call the start function to create the future, and pass it into the program that was started
        let context = SceneContext::new(&self.core, &subprogram);
        let program = with_scene_context(&context, || program(input_stream, context.clone()));

        send_context.send((program, context)).ok();
    }

    ///
    /// Connects the output `stream` of the `source` program to the input of `target`
    ///
    /// Sub-programs can send messages without needing to know what handles them, for instance by creating an output stream using
    /// `scene_context.send(())`. This call provides the means to specify how these streams are connected, for example by
    /// calling `scene.connect_programs((), some_target_program_id, StreamId::with_message_type::<SomeMessageType>())` to connect
    /// everything that sends `SomeMessageType` to the subprogram with the ID `some_target_program_id`.
    ///
    /// The parameters can be used to specify exactly which stream should be redirected: it's possible to redirect only the streams
    /// originating from a specific subprogram, or even streams that requested a particular target. A filtering mechanism is also
    /// provided, in case it's necessary to change the type of the message to suit the target.
    ///
    /// The target is usually a specific program, but can also be `StreamTarget::None` to indicate that any messages should be
    /// dropped with no further action. `StreamTarget::Any` is the default, and will result in the stream blocking until another
    /// call connects it.
    ///
    /// The stream ID specifies which of the streams originating from the souce should be connected. This can either be created
    /// using `StreamId::with_message_type::<SomeMessage>()` to indicate all outgoing streams of that type from `source`, or 
    /// `StreamId::with_message_type::<SomeMessage>().for_target(target)` to indicate an outgoing stream with a specific destination.
    ///
    /// Examples:
    ///
    /// ```
    /// #   use flo_scene::*;
    /// #   use futures::prelude::*;
    /// #   use serde::*;
    /// #
    /// #   #[derive(Serialize, Deserialize)]
    /// #   enum ExampleMessage { Test };
    /// #   impl SceneMessage for ExampleMessage { }
    /// #   #[derive(Serialize, Deserialize)]
    /// #   enum FilteredMessage { Test };
    /// #   impl SceneMessage for FilteredMessage { }
    /// #   let scene           = Scene::empty();
    /// #   let subprogram      = SubProgramId::new();
    /// #   let source_program  = SubProgramId::new();
    /// #   let other_program   = SubProgramId::new();
    /// #   let example_filter  = FilterHandle::for_filter(|input_stream: InputStream<FilteredMessage>| input_stream.map(|_| ExampleMessage::Test));
    /// #
    /// // Connect all the 'ExampleMessage' streams to one program
    /// scene.connect_programs((), &subprogram, StreamId::with_message_type::<ExampleMessage>());
    /// 
    /// // Direct the messages for the source_program to other_program instead (takes priority over the 'any' example set up above)
    /// scene.connect_programs(&source_program, &other_program, StreamId::with_message_type::<ExampleMessage>());
    ///
    /// // Make 'other_program' throw away its messages
    /// scene.connect_programs(&other_program, StreamTarget::None, StreamId::with_message_type::<ExampleMessage>());
    ///
    /// // When 'source_program' tries to connect directly to 'subprogram', send its output to 'other_program' instead
    /// scene.connect_programs(&source_program, &other_program, StreamId::with_message_type::<ExampleMessage>().for_target(&subprogram));
    ///
    /// // Use a filter to accept a different incoming message type for a target program
    /// scene.connect_programs((), StreamTarget::Filtered(example_filter, other_program), StreamId::with_message_type::<FilteredMessage>());
    /// scene.connect_programs(StreamSource::Filtered(example_filter), StreamTarget::Program(other_program), StreamId::with_message_type::<FilteredMessage>());
    ///
    /// // Filter any output if it's connected to an input of a specified type
    /// scene.connect_programs(StreamSource::Filtered(example_filter), (), StreamId::with_message_type::<FilteredMessage>().for_target(&subprogram));
    /// ```
    ///
    pub fn connect_programs(&self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<ConnectionResult, ConnectionError> {
        // Convert the source & target, then pass the request on to the core
        let source = source.into();
        let target = target.into();
        let stream = stream.into();

        SceneCore::connect_programs(&self.core, source, target, stream)
    }

    ///
    /// Creates a stream that can be used to send messages into this scene from elsewhere
    ///
    /// This scene must have a `OUTSIDE_SCENE_PROGRAM` running in order to act as a source for these messages (and this can also be used to
    /// connect or reconnect the streams returned by this function) .
    ///
    pub fn send_to_scene<TMessage>(&self, target: impl Into<StreamTarget>) -> Result<impl Sink<TMessage, Error=SceneSendError<TMessage>>, ConnectionError> 
    where
        TMessage: 'static + SceneMessage,
    {
        let target = target.into();

        SceneCore::initialise_message_type(&self.core, StreamId::with_message_type::<TMessage>());

        // Fetch the outside scene program, which is the source for messages on this stream
        let program_id      = *OUTSIDE_SCENE_PROGRAM;
        let program_core    = self.core.lock().unwrap().get_sub_program(program_id).ok_or(ConnectionError::NoOutsideSceneSubProgram)?;
        let stream_id       = StreamId::with_message_type::<TMessage>().for_target(target.clone());

        // Try to re-use an existing target
        let existing_core = program_core.lock().unwrap().output_core(&stream_id);

        if let Some(existing_core) = existing_core {
            // Reattach to the existing output core
            let output_sink = OutputSink::attach(program_id, existing_core, &self.core);
            Ok(output_sink)
        } else {
            // Create a new target for this message
            let sink_target = SceneCore::sink_for_target::<TMessage>(&self.core, &program_id, target)?;

            // Try to attach it to the program (or just read the old version)
            let new_or_old_target = program_core.lock().unwrap().try_create_output_target(&stream_id, sink_target);
            let new_or_old_target = match new_or_old_target { Ok(new) => new, Err(old) => old };

            // Report the new connection
            let target_program  = OutputSinkCore::target_program_id(&new_or_old_target);
            let update          = if let Some(target_program) = target_program {
                SceneUpdate::Connected(program_id, target_program, stream_id)
            } else {
                SceneUpdate::Disconnected(program_id, stream_id)
            };

            SceneCore::send_scene_updates(&self.core, vec![update]);

            // Create an output sink from the target
            let output_sink = OutputSink::attach(program_id, new_or_old_target, &self.core);
            Ok(output_sink)
        }
    }

    ///
    /// Returns a future that will run any waiting programs on the current thread
    ///
    pub fn run_scene(&self) -> impl Future<Output=()> {
        run_core(&self.core)
    }

    ///
    /// Returns a future that will run the scene across `num_threads` threads (including the thread this is awaited from)
    ///
    /// The subthreads will end when the scene is ended, or the returned future is dropped.
    ///
    pub fn run_scene_with_threads(&self, num_threads: usize) -> impl Future<Output=()> {
        use futures::executor;
        use std::thread::{JoinHandle};
        use std::thread;
        use std::mem;

        // We take a copy of the core to run on the remote threads
        let core = Arc::clone(&self.core);

        // The dropper will stop the child threads when the main thread future is dropped
        struct Dropper {
            /// The senders to signal when this is dropped
            stoppers: Vec<oneshot::Sender<()>>,

            /// The join handles for waiting for the threads to shut down
            join_handles: Vec<JoinHandle<()>>,
        }

        impl Drop for Dropper {
            fn drop(&mut self) {
                // Wake up all the threads and tell them to stop
                for stopper in self.stoppers.drain(..) {
                    stopper.send(()).ok();
                }

                // Wait for all the threads to shut down before finishing the drop
                for join_handle in self.join_handles.drain(..) {
                    join_handle.join().ok();
                }
            }
        }

        async move {
            // The stoppers are used to signal the subthreads to stop when the future is dropped
            let mut stoppers: Vec<oneshot::Sender<()>>  = vec![];
            let mut join_handles                        = vec![];

            for _ in 1..num_threads {
                // Create the channel used to signal the thread to stop
                let (send_stop, recv_stop) = oneshot::channel();

                // Create the thread itself
                let core        = Arc::clone(&core);
                let join_handle = thread::spawn(move || {
                    executor::block_on(async move {
                        // Run the scene until the scene itself stops or the 'stop' event is triggered
                        let scene_runner = run_core(&core);

                        future::select(scene_runner, recv_stop.map(|_| ())).await;
                    });
                });

                // Stopper is signalled when the dropper is dropped, and the join handles are awaited at that time too
                stoppers.push(send_stop);
                join_handles.push(join_handle);
            }

            // The dropper will be dropped when this returned future is done
            let dropper = Dropper { stoppers, join_handles };

            // Run the scene on this thread as well
            run_core(&core).await;

            // Dropper will ensure that all the subthreads are shutdown (if we reach here, or if the future is dropped ahead of time)
            mem::drop(dropper);
        }
    }
}

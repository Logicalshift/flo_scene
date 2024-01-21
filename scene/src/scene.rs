use crate::input_stream::*;
use crate::output_sink::*;
use crate::scene_context::*;
use crate::scene_core::*;
use crate::scene_message::*;
use crate::stream_id::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::subprogram_id::*;
use crate::error::*;
use crate::programs::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::future::{poll_fn};
use futures::{pin_mut};

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
        scene.add_subprogram(*SCENE_CONTROL_PROGRAM, SceneControl::scene_control_program, 0);
        scene.add_subprogram(*OUTSIDE_SCENE_PROGRAM, outside_scene_program, 0);
        SceneCore::set_scene_update_from(&scene.core, *SCENE_CONTROL_PROGRAM);

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
        let input_stream    = InputStream::new(program_id, max_input_waiting);
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
    /// Specifies that an output of `source` (identified by the StreamId) should be connected to the input of `target`
    ///
    /// Streams can be connected either from any program that outputs that particular message type or from a specific program.
    ///
    /// The target is usually a specific program, but can also be `StreamTarget::None` to indicate that any messages should be
    /// dropped with no further action. `StreamTarget::Any` is the default, and will result in the stream blocking until another
    /// call connects it.
    ///
    /// The stream ID specifies which of the streams belonging to the target should be connected: this is usually the `MessageType`
    /// identfier, which will connect a stream that produces data of a known type, but can also be used to redirect a stream that
    /// was going to a particular target.
    ///
    /// Examples:
    ///
    /// ```
    /// #   use flo_scene::*;
    /// #   enum ExampleMessage { Test };
    /// #   impl SceneMessage for ExampleMessage { }
    /// #   let scene           = Scene::empty();
    /// #   let subprogram      = SubProgramId::new();
    /// #   let source_program  = SubProgramId::new();
    /// #   let other_program   = SubProgramId::new();
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
    /// scene.connect_programs(&source_program, &other_program, StreamId::for_target::<ExampleMessage>(&subprogram));
    /// ```
    ///
    pub fn connect_programs(&self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<(), ConnectionError> {
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
    pub fn send_to_scene<TMessage>(&self, target: impl Into<StreamTarget>) -> Result<impl Sink<TMessage, Error=SceneSendError>, ConnectionError> 
    where
        TMessage: 'static + SceneMessage,
    {
        let target = target.into();

        // Fetch the outside scene program, which is the source for messages on this stream
        let program_id      = *OUTSIDE_SCENE_PROGRAM;
        let program_core    = self.core.lock().unwrap().get_sub_program(program_id).ok_or(ConnectionError::NoOutsideSceneSubProgram)?;
        let stream_id       = StreamId::for_target::<TMessage>(target.clone());

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
}

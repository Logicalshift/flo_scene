use crate::input_stream::*;
use crate::scene_context::*;
use crate::scene_core::*;
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

        // Scene control messages are sent to the scene control program by default
        scene.connect_programs((), *SCENE_CONTROL_PROGRAM, StreamId::with_message_type::<SceneControl>()).unwrap();

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
        TProgramFn:     'static + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture,
    {
        // Create the context and input stream for the program
        let input_stream    = InputStream::new(max_input_waiting);
        let input_core      = input_stream.core();

        // Create the future that will be used to run the future
        let (send_context, recv_context) = oneshot::channel::<SceneContext>();
        let run_program = async move {
            if let Ok(scene_context) = recv_context.await {
                // Start the program running
                let program = with_scene_context(&scene_context, || program(input_stream, scene_context.clone()));
                pin_mut!(program);

                // Poll the program with the scene context set
                poll_fn(|mut context| {
                    with_scene_context(&scene_context, || {
                        program.as_mut().poll(&mut context)
                    })
                }).await;
            }
        };

        // Start the program running
        let subprogram = SceneCore::start_subprogram(&self.core, program_id, run_program, input_core);

        // Create the scene context, and send it to the subprogram
        let context = SceneContext::new(&self.core, &subprogram);
        send_context.send(context).ok();
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
    /// Returns a future that will run any waiting programs on the current thread
    ///
    pub fn run_scene(&self) -> impl Future<Output=()> {
        run_core(&self.core)
    }
}

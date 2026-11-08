use crate::host::error::*;
use crate::host::scene_core::*;
use crate::host::input_stream::*;
use crate::host::scene_context::*;
use crate::host::stream_type_functions::*;

use flo_scene_messages::prelude::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::{pin_mut};

use std::any::{Any};
use std::panic::{AssertUnwindSafe};
use std::sync::*;

///
/// Concreate initialisation context for a scene
///
pub struct SceneInitialisationContext {
    scene_core: Arc<Mutex<SceneCore>>,
}

impl InitialisationContext for SceneInitialisationContext {
    fn get_init_context<TContextType: 'static>(&mut self) -> Option<&mut TContextType> {
        (self as &mut dyn Any).downcast_mut::<TContextType>()
    }
}

///
/// The initialisation context is used when setting up scenes and messages within scenes: it provides
/// routines for creating and connecting programs.
///
/// Once the scene is running, the scene control program can be used to similar effect (by sending 
/// `SceneControl` messages)
///
impl SceneInitialisationContext {
    ///
    /// Adds a subprogram to run in this scene
    ///
    pub fn add_subprogram<'a, TProgramFn, TInputMessage, TFuture>(&'a self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize)
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'a + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture
    {
        StreamTypeFunctions::add::<TInputMessage>();

        // Create the context and input stream for the program
        let input_stream    = InputStream::new(program_id, &self.scene_core, max_input_waiting);
        let input_core      = input_stream.core();

        // Create the future that will be used to run the future
        let (send_context, recv_context) = oneshot::channel::<(TFuture, SceneContext)>();
        let run_program = async move {
            if let Ok((program, scene_context)) = recv_context.await {
                // Start the program running
                pin_mut!(program);

                // Poll the program with the scene context set
                future::poll_fn(|context| {
                    use futures::task::{Poll};
                    match with_scene_context(&scene_context, AssertUnwindSafe(|| program.as_mut().poll(context))) {
                        Ok(result)  => result,
                        Err(())     => Poll::Pending
                    }
                }).await;
            }
        };

        // Start the program running
        let subprogram = SceneCore::start_subprogram(&self.scene_core, program_id, run_program, input_core);

        // Call the start function to create the future, and pass it into the program that was started
        let context = SceneContext::new(&self.scene_core, &subprogram);
        let program = with_scene_context(&context, AssertUnwindSafe(|| program(input_stream, context.clone())));

        if let Ok(program) = program { send_context.send((program, context)).ok(); }
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
    /// scene.connect_programs((), StreamTarget::Filtered(example_filter.clone(), other_program), StreamId::with_message_type::<FilteredMessage>());
    /// scene.connect_programs(&example_filter, StreamTarget::Program(other_program), StreamId::with_message_type::<FilteredMessage>());
    ///
    /// // Filter any output if it's connected to an input of a specified type
    /// scene.connect_programs(&example_filter, (), StreamId::with_message_type::<FilteredMessage>().for_target(&subprogram));
    /// ```
    ///
    pub fn connect_programs(&self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<ConnectionResult, ConnectionError> {
        // Convert the source & target, then pass the request on to the core
        let source = source.into();
        let target = target.into();
        let stream = stream.into();

        SceneCore::connect_programs(&self.scene_core, source, target, stream)
    }
}

pub trait SceneInitialisationExt {
    ///
    /// Adds a subprogram to run in this scene
    ///
    fn add_subprogram<'a, TProgramFn, TInputMessage, TFuture>(&'a mut self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Result<(), ConnectionError>
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'a + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture;

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
    /// scene.connect_programs((), StreamTarget::Filtered(example_filter.clone(), other_program), StreamId::with_message_type::<FilteredMessage>());
    /// scene.connect_programs(&example_filter, StreamTarget::Program(other_program), StreamId::with_message_type::<FilteredMessage>());
    ///
    /// // Filter any output if it's connected to an input of a specified type
    /// scene.connect_programs(&example_filter, (), StreamId::with_message_type::<FilteredMessage>().for_target(&subprogram));
    /// ```
    ///
    fn connect_programs(&mut self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<ConnectionResult, ConnectionError>;
}

impl<T: InitialisationContext> SceneInitialisationExt for T {
    #[inline]
    fn add_subprogram<'a, TProgramFn, TInputMessage, TFuture>(&'a mut self, program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Result<(), ConnectionError>
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'a + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture
    {
        if let Some(context) = self.get_init_context::<SceneInitialisationContext>() {
            context.add_subprogram(program_id, program, max_input_waiting);
            Ok(())
        } else {
            Err(ConnectionError::Cancelled)
        }
    }

    #[inline]
    fn connect_programs(&mut self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<ConnectionResult, ConnectionError> {
        if let Some(context) = self.get_init_context::<SceneInitialisationContext>() {
            context.connect_programs(source, target, stream)
        } else {
            Err(ConnectionError::Cancelled)
        }
    }
}

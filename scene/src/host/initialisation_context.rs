use crate::host::connect_result::*;
use crate::host::error::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::stream_id::*;

///
/// The initialisation context is used when setting up scenes and messages within scenes: it provides
/// routines for creating and connecting programs.
///
/// Once the scene is running, the scene control program can be used to similar effect (by sending 
/// `SceneControl` messages)
///
pub trait SceneInitialisationContext {
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
    fn connect_programs(&self, source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream: impl Into<StreamId>) -> Result<ConnectionResult, ConnectionError>;
}

use crate::error::*;
use crate::filter::*;
use crate::input_stream::*;
use crate::scene_context::*;
use crate::scene::*;
use crate::scene_core::*;
use crate::scene_message::*;
use crate::stream_id::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::subprogram_id::*;

use super::idle_request::*;
use super::subscription::*;
use super::query::*;

use futures::prelude::*;
use futures::future::{poll_fn};
use futures::channel::oneshot;
use futures::stream;
use futures::{pin_mut};

use once_cell::sync::{Lazy};

use std::collections::{HashSet, HashMap};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::sync::*;

/// The identifier for the standard scene control program
pub static SCENE_CONTROL_PROGRAM: StaticSubProgramId = StaticSubProgramId::called("flo_scene::scene_control");

/// Filter that maps the 'Subscribe' message to a SceneControl message
static SCENE_CONTROL_SUBSCRIBE_FILTER: Lazy<FilterHandle> = Lazy::new(|| FilterHandle::for_filter(|stream: InputStream<Subscribe<SceneUpdate>>| stream.map(|msg| SceneControl::Subscribe(msg.target()))));

/// Filter that maps the 'Query' message to a SceneControl message
static SCENE_CONTROL_QUERY_FILTER: Lazy<FilterHandle> = Lazy::new(|| FilterHandle::for_filter(|stream: InputStream<Query<SceneUpdate>>| stream.map(|msg| SceneControl::Query(msg.target()))));

///
/// Represents a program start function
///
pub struct SceneProgramFn(Box<dyn Send + FnOnce(Arc<Mutex<SceneCore>>)>);

///
/// Messages that can be sent to the main scene control program
///
#[derive(Debug)]
pub enum SceneControl {
    ///
    /// Starts a new sub-program in this scene
    ///
    Start(SceneProgramFn),

    ///
    /// Sets up a connection between the output of a source to a target. The StreamId identifies which output in the
    /// source is being connected.
    ///
    Connect(StreamSource, StreamTarget, StreamId),

    ///
    /// Marks the input stream for a subprogram as 'closed', which will usually cause it to shut down
    ///
    /// This allows a program to shut down gracefully
    ///
    Close(SubProgramId),

    ///
    /// Waits for all of the subprograms in the scene to process all of their remaining messages and then stops the scene
    ///
    /// This is a less abrupt verison of 'StopScene' that will ensure that all of the subprograms have no pending messages
    /// and are waiting for new messages to arrive before shutting the scene down: typically this will ensure that all
    /// of the messages in the scene have been fully processed before the scene is stopped.
    ///
    /// If something in the scene is blocked waiting for something, or something is constantly generating messages, this 
    /// may never actually stop the scene. `StopScene` is a bit more forceful and will terminate the scene at the point
    /// where all the executing futures have yielded.
    ///
    StopSceneWhenIdle,

    ///
    /// Terminates the entire scene by stopping any calls to `run_scene()`
    ///
    /// This will interrupt any in-progress task at the point where it most recently yielded. Use `StopSceneWhenIdle`
    /// to shut down the scene when all the in progress messages have been completed.
    ///
    StopScene,

    ///
    /// Subscribes the specified program to `SceneUpdate` events from the controller. This will send messages for the
    /// current state of the control before the new messages so the entire state can be determined
    ///
    Subscribe(StreamTarget),

    ///
    /// Sends the updates as a QueryResponse<SceneUpdate> to the specified subprogram
    ///
    /// Queries respond with the list of running programs and connections at the time of connections. Note that a query can
    /// sometimes return programs that haven't yet sent their notifications to subscribers.
    ///
    Query(StreamTarget),
}

// TODO: make the scene updates serializable (needs StreamId to be serializable first)

///
/// Messages generated by the control program
///
#[derive(Clone, Debug, PartialEq)]
pub enum SceneUpdate {
    /// A subprogram that receives a particular type of input stream has started
    Started(SubProgramId, StreamId),

    /// The output specified by the stream ID for the first subprogram has been connected to the input for the second
    Connected(SubProgramId, SubProgramId, StreamId),

    /// The output specified by the stream ID has been disconnected
    Disconnected(SubProgramId, StreamId),

    /// A requested connection failed to be made for some reason
    FailedConnection(ConnectionError, StreamSource, StreamTarget, StreamId),

    /// A subprogram has finished running
    Stopped(SubProgramId),
}

impl SceneProgramFn {
    ///
    /// Creates a new SceneProgramFn that will start a subprogram in a scene
    ///
    pub fn new<TProgramFn, TInputMessage, TFuture>(program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Self
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'static + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture,
    {
        // TODO: this is almost the same 'start' procedure as appears in the main 'Scene' type (modified because control requests are cloneable so the start function has to be 'Sync')
        let start_fn    = move |scene_core: Arc<Mutex<SceneCore>>| {
            // Create the context and input stream for the program
            let input_stream    = InputStream::new(program_id, &scene_core, max_input_waiting);
            let input_core      = input_stream.core();

            // Create the future that will be used to run the future
            let (send_context, recv_context)    = oneshot::channel::<(TFuture, SceneContext)>();
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
            let subprogram = SceneCore::start_subprogram(&scene_core, program_id, run_program, input_core);

            // Create the scene context, and send it to the subprogram
            let context = SceneContext::new(&scene_core, &subprogram);
            let program = program(input_stream, context.clone());
            send_context.send((program, context)).ok();
        };

        // Turn the function into a SceneProgramFn
        let start_fn: Box<dyn Send + FnOnce(Arc<Mutex<SceneCore>>)> = Box::new(start_fn);
        SceneProgramFn(Box::new(start_fn))
    }

    ///
    /// Adds the program that is started by this function to a scene
    ///
    #[inline]
    pub fn add_to_scene(self, scene: &Scene) {
        (self.0)(Arc::clone(scene.core()))
    }
}

impl Debug for SceneProgramFn {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "SceneProgramFn(...)")
    }
}

impl SceneMessage for SceneControl { 
    fn default_target() -> StreamTarget {
        // Send control messages to the main control program by default
        (*SCENE_CONTROL_PROGRAM).into()
    }

    fn initialise(scene: &Scene) {
        scene.connect_programs(StreamSource::Filtered(*SCENE_CONTROL_SUBSCRIBE_FILTER), (), StreamId::with_message_type::<Subscribe<SceneUpdate>>()).unwrap();
        scene.connect_programs(StreamSource::Filtered(*SCENE_CONTROL_QUERY_FILTER), (), StreamId::with_message_type::<Query<SceneUpdate>>()).unwrap();

        // TODO: this is done in the scene 'with_standard_programs' right now because you can't connect before a program is added
        // scene.connect_programs((), *SCENE_CONTROL_PROGRAM, StreamId::with_message_type::<Subscribe<SceneUpdate>>()).unwrap();
    }
}

impl SceneMessage for SceneUpdate { 
    fn default_target() -> StreamTarget {
        // Updates are discarded by default
        StreamTarget::None
    }
}

impl SceneControl {
    ///
    /// Creates a start program message for the scene control subprogram
    ///
    pub fn start_program<TProgramFn, TInputMessage, TFuture>(program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Self
    where
        TFuture:        'static + Send + Future<Output=()>,
        TInputMessage:  'static + SceneMessage,
        TProgramFn:     'static + Send + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture,
    {
        let start_fn = SceneProgramFn::new(program_id, program, max_input_waiting);
        SceneControl::Start(start_fn)
    }

    ///
    /// Creates a 'connect' message
    ///
    pub fn connect(source: impl Into<StreamSource>, target: impl Into<StreamTarget>, stream_id: impl Into<StreamId>) -> Self {
        SceneControl::Connect(source.into(), target.into(), stream_id.into())
    }

    ///
    /// Runs the scene control program
    ///
    pub (crate) async fn scene_control_program(input: InputStream<Self>, context: SceneContext, updates: InputStream<SceneUpdate>) {
        // We store the state by monitoring the updates (used to respond to queries or new subscription requests)
        // This state is kept separate from the scene core state so that if we're starting a subscription we won't send a pending update more than once (ie, the events we've sent can be out of date with respect to the actual scene core state)
        let mut started_subprograms = HashSet::<SubProgramId>::new();
        let mut active_connections  = HashMap::<(SubProgramId, StreamId), SubProgramId>::new();

        // Most of the scene control program's functionality is performed by manipulating the scene core directly
        let scene_core              = context.scene_core();
        let mut update_subscribers  = EventSubscribers::new();

        update_subscribers.add_target(context.send::<SceneUpdate>(StreamTarget::None).unwrap());

        // We read from the update stream and the input stream at the same time
        enum ControlInput {
            Control(SceneControl),
            Update(SceneUpdate),
        }

        let input   = input.map(|input| ControlInput::Control(input));
        let updates = updates.map(|update| ControlInput::Update(update));

        // The program runs until the input is exhausted
        let mut input = stream::select(input, updates);
        while let Some(request) = input.next().await {
            use SceneControl::*;
            use ControlInput::*;

            match request {
                Control(Start(start_fn)) => {
                    // Downcast the start function and call it
                    if let Some(scene_core) = scene_core.upgrade() {
                        let start_fn = start_fn.0;

                        (start_fn)(scene_core);
                    } else {
                        break;
                    }
                },

                Control(Connect(source, target, stream)) => {
                    if let Some(scene_core) = scene_core.upgrade() {
                        // Try to connect the program and send an update if the sending failed
                        match SceneCore::connect_programs(&scene_core, source.clone(), target.clone(), stream.clone()) {
                            Ok(())      => { }
                            Err(error)  => {
                                update_subscribers.send(SceneUpdate::FailedConnection(error, source, target, stream)).await;
                            }
                        }
                    } else {
                        break;
                    }
                },

                Control(Close(sub_program_id)) => {
                    // Try to close the input stream for a subprogram
                    if let Some(scene_core) = scene_core.upgrade() {
                        let waker = {
                            let program     = scene_core.lock().unwrap().get_sub_program(sub_program_id);
                            let input_core  = scene_core.lock().unwrap().get_input_stream_core(sub_program_id);

                            if let (Some(program), Some(input_core)) = (program, input_core) {
                                let input_stream_id = program.lock().unwrap().input_stream_id();

                                input_stream_id.close_input(&input_core)
                            } else {
                                Ok(None)
                            }
                        };

                        if let Ok(Some(waker)) = waker {
                            waker.wake()
                        }
                    }
                },

                Control(StopSceneWhenIdle) => {
                    // Start a new subprogram that requests an idle notification, then relays the 'stop' message back to us
                    let idle_program    = SubProgramId::new();
                    let scene_control   = context.current_program_id().unwrap();

                    let wait_for_idle   = SceneProgramFn::new(idle_program, move |input: InputStream<IdleNotification>, context| async move {
                        // Request an idle notification
                        if context.send_message(IdleRequest::WhenIdle(idle_program)).await.is_ok() {
                            // Wait for the notification to arrive
                            let mut input = input;
                            input.allow_thread_stealing(true);
                            input.next().await;
                        }

                        // Tell the control program to shut down once the idle message arrives
                        if let Ok(mut scene_control) = context.send::<SceneControl>(scene_control) {
                            scene_control.send(SceneControl::StopScene).await.ok();
                        }
                    }, 0);

                    if let Some(scene_core) = scene_core.upgrade() {
                        (wait_for_idle.0)(scene_core);
                    }
                },

                Control(StopScene) => {
                    if let Some(scene_core) = scene_core.upgrade() {
                        // Tell the core to stop (note: awaits won't return at this point!)
                        let wakers = scene_core.lock().unwrap().stop();

                        // Wake all the threads so they stop the core
                        wakers.into_iter().for_each(|waker| waker.wake());
                    }
                },

                Control(Subscribe(target)) => {
                    // Add to the subscribers
                    update_subscribers.subscribe(&context, target.clone());

                    // Send the current state
                    if let (Ok(mut subscriber), Some(scene_core)) = (context.send(target), scene_core.upgrade()) {
                        // Indicate all the programs have started
                        for prog in started_subprograms.iter() {
                            let prog            = *prog;
                            let subprogram_core = scene_core.lock().unwrap().get_sub_program(prog);

                            if let Some(subprogram_core) = subprogram_core {
                                let input_stream_id = subprogram_core.lock().unwrap().input_stream_id.clone();

                                subscriber.send(SceneUpdate::Started(prog, input_stream_id)).await.ok();
                            }
                        }

                        // Send all of the connections
                        for ((source, stream), target) in active_connections.iter() {
                            subscriber.send(SceneUpdate::Connected(*source, *target, stream.clone())).await.ok();
                        }
                    }
                },

                Control(Query(target)) => {
                    // Send a query response to the target
                    if let (Ok(mut query_response), Some(scene_core)) = (context.send(target), scene_core.upgrade()) {
                        // Build a response out of the current state of the scene
                        let running_subprograms = scene_core.lock().unwrap().get_running_subprograms();

                        // Fetching the full list of active connections is a little more involved than the list of programs
                        let active_connections =
                            running_subprograms.iter()
                                .flat_map(|program_id| scene_core.lock().unwrap().get_sub_program(*program_id))
                                .flat_map(|program_core| {
                                    use std::mem;

                                    // Fetch the list of output streams for this program
                                    let program_core    = program_core.lock().unwrap();
                                    let program_id      = *program_core.program_id();
                                    let output_streams  = program_core.output_streams().map(|(stream, sink)| (stream.clone(), sink.clone())).collect::<Vec<_>>();

                                    // Figure out their currently connected targets (unlock the core to avoid deadlocks)
                                    mem::drop(program_core);

                                    // Fetch the targets for each stream and map to subprogram IDs when known
                                    let output_streams = output_streams.into_iter()
                                        .map(|(stream_id, output_sink)| {
                                            let target = stream_id.active_target_for_output_sink(&output_sink);

                                            (stream_id, target)
                                        })
                                        .flat_map(|(stream_id, target)| {
                                            match target {
                                                Ok(StreamTarget::Program(program_id))       => Some((stream_id, program_id)),
                                                Ok(StreamTarget::Filtered(_, program_id))   => Some((stream_id, program_id)),
                                                _                                           => None,
                                            }
                                        });


                                    output_streams.map(move |connection| {
                                        (program_id, connection)
                                    })
                                })
                                .collect::<Vec<_>>();

                        let response = running_subprograms.iter()
                            .flat_map(|prog| scene_core.lock().unwrap().get_sub_program(*prog).map(|core| (prog, core)))
                            .map(|(prog, core)| SceneUpdate::Started(*prog, core.lock().unwrap().input_stream_id.clone()))
                            //.chain(active_connections.iter().map(|((source, stream_id), target)| SceneUpdate::Connected(*source, *target, stream_id.clone())))
                            .chain(active_connections.iter().map(|(source, (stream_id, target))| SceneUpdate::Connected(*source, *target, stream_id.clone())))
                            .collect::<Vec<_>>();

                        // Send as a query response
                        query_response.send(QueryResponse::with_stream(stream::iter(response))).await.ok();
                    }
                }

                Update(update) => {
                    // Update our internal state
                    match &update {
                        SceneUpdate::Started(program_id, _input_stream_id)  => { started_subprograms.insert(*program_id); },
                        SceneUpdate::Connected(source, target, stream_id)   => { active_connections.insert((*source, stream_id.clone()), *target); },
                        SceneUpdate::Disconnected(source, stream_id)        => { active_connections.remove(&(*source, stream_id.clone())); },
                        SceneUpdate::Stopped(program_id)                    => { started_subprograms.remove(program_id); },

                        SceneUpdate::FailedConnection(_, _, _, _)           => { },
                    }

                    // Send the update to the subscribers
                    update_subscribers.send(update).await;
                }
            }
        }
    }
}

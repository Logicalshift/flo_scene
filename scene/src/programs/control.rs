use crate::error::*;
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

use futures::prelude::*;
use futures::future::{poll_fn};
use futures::channel::oneshot;
use futures::{pin_mut};
use once_cell::sync::{Lazy};

use std::fmt;
use std::fmt::{Debug, Formatter};
use std::sync::*;

/// The identifier for the standard scene control program
pub static SCENE_CONTROL_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("SCENE_CONTROL_PROGRAM"));

///
/// Represents a program start function
///
pub struct SceneProgramFn(Box<dyn Send + Sync + FnOnce(Arc<Mutex<SceneCore>>)>);

///
/// Messages that can be sent to the main scene control program
///
#[derive(Debug)]
pub enum SceneControl {
    ///
    /// Starts a new sub-program in this scene
    ///
    Start(SubProgramId, SceneProgramFn),

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
}

///
/// Messages generated by the control program
///
#[derive(Clone, Debug)]
pub enum SceneUpdate {
    /// A subprogram has started
    Started(SubProgramId),

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
        TProgramFn:     'static + Send + Sync + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture,
    {
        // TODO: this is almost the same 'start' procedure as appears in the main 'Scene' type (modified because control requests are cloneable so the start function has to be 'Sync')
        let start_fn    = move |scene_core: Arc<Mutex<SceneCore>>| {
            // Create the context and input stream for the program
            let input_stream    = InputStream::new(program_id, max_input_waiting);
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
        let start_fn: Box<dyn Send + Sync + FnOnce(Arc<Mutex<SceneCore>>)> = Box::new(start_fn);
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
        // Send to the main control program by default
        (*SCENE_CONTROL_PROGRAM).into()
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
        TProgramFn:     'static + Send + Sync + FnOnce(InputStream<TInputMessage>, SceneContext) -> TFuture,
    {
        let start_fn = SceneProgramFn::new(program_id, program, max_input_waiting);
        SceneControl::Start(program_id, start_fn)
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
    pub (crate) async fn scene_control_program(input: InputStream<Self>, context: SceneContext) {
        // Most of the scene control program's functionality is performed by manipulating the scene core directly
        let scene_core  = context.scene_core();
        let mut updates = context.send::<SceneUpdate>(StreamTarget::None).unwrap();

        // The program runs until the input is exhausted
        let mut input = input;
        while let Some(request) = input.next().await {
            use SceneControl::*;

            match request {
                Start(_program_id, start_fn) => {
                    // Downcast the start function and call it
                    if let Some(scene_core) = scene_core.upgrade() {
                        let start_fn = start_fn.0;

                        (start_fn)(scene_core);
                    } else {
                        break;
                    }
                },

                Connect(source, target, stream) => {
                    if let Some(scene_core) = scene_core.upgrade() {
                        // Try to connect the program and send an update if the sending failed
                        match SceneCore::connect_programs(&scene_core, source.clone(), target.clone(), stream.clone()) {
                            Ok(())      => { }
                            Err(error)  => {
                                updates.send(SceneUpdate::FailedConnection(error, source, target, stream)).await.ok();
                            }
                        }
                    } else {
                        break;
                    }
                },

                Close(sub_program_id) => {
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

                StopSceneWhenIdle => {
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

                StopScene => {
                    if let Some(scene_core) = scene_core.upgrade() {
                        // Tell the core to stop (note: awaits won't return at this point!)
                        let wakers = scene_core.lock().unwrap().stop();

                        // Wake all the threads so they stop the core
                        wakers.into_iter().for_each(|waker| waker.wake());
                    }
                },
            }
        }
    }
}

use crate::error::*;
use crate::input_stream::*;
use crate::scene_context::*;
use crate::scene_core::*;
use crate::stream_id::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::future::{poll_fn};
use futures::channel::oneshot;
use futures::{pin_mut};
use once_cell::sync::{Lazy};

use std::any::*;
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::sync::*;

/// The identifier for the standard scene control program
pub static SCENE_CONTROL_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("SCENE_CONTROL_PROGRAM"));

///
/// Represents a program start function
///
#[derive(Clone)]
pub struct SceneProgramFn(Arc<dyn Send + Sync + Any>);

///
/// Messages that can be sent to the main scene control program
///
#[derive(Clone, Debug)]
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
    /// Terminates the entire scene by stopping any calls to `run_scene()`
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

    /// A requested connection failed to be made for some reason
    FailedConnection(ConnectionError, StreamSource, StreamTarget, StreamId),

    /// A subprogram has finished running
    Stopped(SubProgramId),
}

impl Debug for SceneProgramFn {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "SceneProgramFn(...)")
    }
}

impl SceneControl {
    ///
    /// Creates a start program message for the scene control subprogram
    ///
    pub fn start_program<TProgramFn, TInputMessage, TFuture>(program_id: SubProgramId, program: TProgramFn, max_input_waiting: usize) -> Self
    where
        TFuture:        Send + Sync + Future<Output=()>,
        TInputMessage:  'static + Unpin + Send + Sync,
        TProgramFn:     'static + Send + Sync + Fn(InputStream<TInputMessage>, SceneContext) -> TFuture,
    {
        // TODO: this is almost the same 'start' procedure as appears in the main 'Scene' type (modified because control requests are cloneable so the start function has to be 'Sync')
        let program     = Arc::new(program);
        let start_fn    = move |scene_core: Arc<Mutex<SceneCore>>| {
            // Create the context and input stream for the program
            let input_stream    = InputStream::new(max_input_waiting);
            let input_core      = input_stream.core();

            // Create the future that will be used to run the future
            let (send_context, recv_context)    = oneshot::channel::<SceneContext>();
            let program                         = Arc::clone(&program);
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
            let subprogram = SceneCore::start_subprogram(&scene_core, program_id, run_program, input_core);

            // Create the scene context, and send it to the subprogram
            let context = SceneContext::new(&scene_core, &subprogram);
            send_context.send(context).ok();
        };

        // Turn the function into a SceneProgramFn
        let start_fn: Box<dyn Send + Sync + Fn(Arc<Mutex<SceneCore>>) -> ()> = Box::new(start_fn);
        let start_fn = SceneProgramFn(Arc::new(start_fn));

        // Wrap this in a message
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
                        let start_fn = start_fn.0.downcast::<Box<dyn Send + Sync + Fn(Arc<Mutex<SceneCore>>) -> ()>>();

                        if let Ok(start_fn) = start_fn {
                            (*start_fn)(scene_core);
                        }
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
                            let program = scene_core.lock().unwrap().get_sub_program(sub_program_id);

                            if let Some(program) = program {
                                program.lock().unwrap().close()
                            } else {
                                None
                            }
                        };

                        if let Some(waker) = waker {
                            waker.wake()
                        }
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

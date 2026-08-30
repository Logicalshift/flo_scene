use crate::host::filter::*;
use crate::host::initialisation_context::*;
use crate::host::input_stream::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::subprogram_id::*;
use crate::host::stream_target::*;

use super::control::*;
use super::control_ext::*;
use super::subscription::*;

use futures::prelude::*;
use serde::*;

use std::borrow::*;
use std::collections::*;
use std::sync::*;

/// The identifier for the standard scene error program
pub static SCENE_ERROR_PROGRAM: StaticSubProgramId = StaticSubProgramId::called("flo_scene::error");

///
/// Message sent to indicate that an error has occurred with a subprogram
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Error {
    /// A non-fatal error has occurred from the specified subprogram
    Error { source: SubProgramId, message: Cow<'static, str> },

    /// An error has occurred and the error program should cause the scene to shut down
    Failure { source: SubProgramId, message: Cow<'static, str> },
}

///
/// Message sent to subscribe to error messages occurring within a scene
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorSubscription {
    /// Send errors destined for the specified subprogram to the specified StreamTarget
    Subscribe(SubProgramId, StreamTarget),

    /// Send all errors to the specified stream target
    SubscribeToAll(StreamTarget),
}

impl SceneMessage for Error {
    fn default_target() -> StreamTarget {
        (*SCENE_ERROR_PROGRAM).into()
    }

    #[inline]
    fn message_type_name() -> String { "flo_scene::Error".into() }

    fn initialise(scene: &impl SceneInitialisationContext) {
        scene.connect_programs(
            (), 
            StreamTarget::Filtered(FilterHandle::for_filter(|msgs| 
                msgs.map(|msg| ErrorOrSubscription::ErrorMsg(msg))),*SCENE_ERROR_PROGRAM), 
            StreamId::with_message_type::<Subscribe<Error>>()
        ).unwrap();
    }
}

impl SceneMessage for ErrorSubscription {
    fn default_target() -> StreamTarget {
        (*SCENE_ERROR_PROGRAM).into()
    }

    #[inline]
    fn message_type_name() -> String { "flo_scene::ErrorSubscription".into() }

    fn initialise(scene: &impl SceneInitialisationContext) {
        scene.connect_programs(
            (), 
            StreamTarget::Filtered(FilterHandle::for_filter(|msgs: InputStream<Subscribe<Error>>| 
                msgs.map(|msg| ErrorOrSubscription::Subscription(ErrorSubscription::SubscribeToAll(msg.target())))),*SCENE_ERROR_PROGRAM), 
            StreamId::with_message_type::<Subscribe<Error>>()
        ).unwrap();

        scene.connect_programs(
            (), 
            StreamTarget::Filtered(FilterHandle::for_filter(|msgs| 
                msgs.map(|msg| ErrorOrSubscription::Subscription(msg))),*SCENE_ERROR_PROGRAM), 
            StreamId::with_message_type::<Subscribe<Error>>()
        ).unwrap();
    }
}

///
/// Combined error or subscription message
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub (crate) enum ErrorOrSubscription {
    ErrorMsg(Error),
    Subscription(ErrorSubscription)
}

impl SceneMessage for ErrorOrSubscription {

}

impl Error {
    ///
    /// Runs the default error handling program for a scene
    ///
    pub (crate) async fn default_error_program(input: InputStream<ErrorOrSubscription>, context: SceneContext) {
        context.i_am("Error program");
        context.tag(SceneProgramTag::Namespace("flo_scene".into())).ok();

        let program_subscribers = Arc::new(Mutex::new(HashMap::new()));
        let all_subscribers     = Arc::new(Mutex::new(EventSubscribers::new()));

        // Listen for possible error events
        let mut input           = input;
        let mut failure_count   = 0;

        use ErrorOrSubscription::*;

        while let Some(error) = input.next().await {
            match error {
                Subscription(ErrorSubscription::Subscribe(subprogram_id, stream_target)) => {
                    program_subscribers
                        .lock().unwrap()
                        .entry(subprogram_id)
                        .or_insert_with(|| EventSubscribers::new())
                        .subscribe(&context, stream_target);
                },

                Subscription(ErrorSubscription::SubscribeToAll(stream_target)) => {
                    all_subscribers
                        .lock().unwrap()
                        .subscribe(&context, stream_target);
                },

                ErrorMsg(Error::Error { source, message }) => {
                    // Send to all subscribers
                    if let Some(program_subscriber) = program_subscribers.lock().unwrap().get_mut(&source) {
                        if !program_subscriber.send(Error::Failure { source, message: message.clone() }).await {
                            // Remove from the list of subscribers (locks can't be held across awaits, so the lock is not held here)
                            let mut program_subscribers = program_subscribers.lock().unwrap();
                            program_subscribers.remove(&source);
                        }
                    }

                    all_subscribers
                        .lock().unwrap()
                        .send(Error::Error { source, message })
                        .await;
                },

                ErrorMsg(Error::Failure { source, message }) => {
                    failure_count += 1;

                    // Send to all subscribers
                    if let Some(program_subscriber) = program_subscribers.lock().unwrap().get_mut(&source) {
                        if !program_subscriber.send(Error::Failure { source, message: message.clone() }).await {
                            // Remove from the list of subscribers (locks can't be held across awaits, so the lock is not held here)
                            let mut program_subscribers = program_subscribers.lock().unwrap();
                            program_subscribers.remove(&source);
                        }
                    }

                    all_subscribers
                        .lock().unwrap()
                        .send(Error::Failure { source, message })
                        .await;

                    // Shut down the scene
                    if failure_count > 1 {
                        // Ask the scene to stop politely
                        context.send_message(SceneControl::StopSceneWhenIdle).await.ok();
                    } else {
                        // Stop impolitely as the scene is producing multiple fatal errors
                        context.send_message(SceneControl::StopScene).await.ok();
                    }
                },
            }
        }
    }
}
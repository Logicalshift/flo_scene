use crate::host::input_stream::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
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
}

impl Error {
    ///
    /// Runs the default error handling program for a scene
    ///
    pub async fn default_error_program(input: InputStream<Error>, context: SceneContext) {
        context.i_am("Error program");
        context.tag(SceneProgramTag::Namespace("flo_scene".into())).ok();

        let program_subscribers = Arc::new(Mutex::new(HashMap::new()));
        let all_subscribers     = Arc::new(Mutex::new(EventSubscribers::new()));

        // Listen for possible error events
        let mut input           = input;
        let mut failure_count   = 0;

        while let Some(error) = input.next().await {
            match error {
                Error::Subscribe(subprogram_id, stream_target) => {
                    program_subscribers
                        .lock().unwrap()
                        .entry(subprogram_id)
                        .or_insert_with(|| EventSubscribers::new())
                        .subscribe(&context, stream_target);
                },

                Error::SubscribeToAll(stream_target) => {
                    all_subscribers
                        .lock().unwrap()
                        .subscribe(&context, stream_target);
                },

                Error::Error { source, message } => {
                    // Send to all subscribers
                    if let Some(program_subscriber) = program_subscribers.lock().unwrap().get_mut(&source) {
                        program_subscriber.send(Error::Error { source, message: message.clone() }).await;
                    }

                    all_subscribers
                        .lock().unwrap()
                        .send(Error::Error { source, message })
                        .await;
                },

                Error::Failure { source, message } => {
                    failure_count += 1;

                    // Send to all subscribers
                    if let Some(program_subscriber) = program_subscribers.lock().unwrap().get_mut(&source) {
                        program_subscriber.send(Error::Failure { source, message: message.clone() }).await;
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
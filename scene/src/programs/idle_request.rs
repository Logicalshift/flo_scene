use crate::*;

use futures::prelude::*;

use once_cell::sync::{Lazy};

use std::collections::{HashMap};

///
/// ID of the program that sends idle notifications by default
///
pub static IDLE_NOTIFICATION_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("IDLE_REQUEST"));

///
/// Idle requests are a way to request a callback that is made when the scene is next idle
///
/// A scene is considered 'idle' when all input streams are waiting with 0 messages remaining. 
///
/// One use for this is for triggering UI rendering after waiting for a state update to process: this
/// will trigger after all commands have finished processing, which could indicate that the UI is
/// now in a state where it can be rendered without further updates ocurring.
///
pub enum IdleRequest {
    ///
    /// When the scene next becomes idle, send a message to the specified subprogram ID
    ///
    WhenIdle(SubProgramId),

    ///
    /// Suppress any idle notifications even if the scene otherwise becomes idle
    ///
    SuppressNotifications,

    ///
    /// Resume the notifications that were supressed by the call to SuppressNotifications
    ///
    ResumeNotifications,
}

///
/// Message sent when the scene becomes idle, after a request is sent to IdleRequest
///
pub struct IdleNotification;

impl SceneMessage for IdleRequest {
    fn default_target() -> StreamTarget { (*IDLE_NOTIFICATION_PROGRAM).into() }

    fn allow_thread_stealing_by_default() -> bool { true }
}

impl SceneMessage for IdleNotification {
    fn default_target() -> StreamTarget { StreamTarget::None }
}

///
/// Runs the idle notification program 
///
pub (crate) async fn idle_program(input_stream: InputStream<IdleRequest>, context: SceneContext) {
    let mut input_stream            = input_stream.messages_with_sources();
    let mut suppressions            = HashMap::new();
    let mut pending_notifications   = vec![];

    while let Some((sender_id, request)) = input_stream.next().await {
        use IdleRequest::*;

        match request {
            WhenIdle(send_message_to) => {
                // Add to the list of subprograms to send a message to when an idle request comes in (we'll send multiple notifications if the same program has requested them)
                pending_notifications.push(send_message_to); 

                // TODO: set the core to notify us when idle
            },

            SuppressNotifications => {
                // Each program gets its own suppression count, so they can't undo the suppressions of other programs
                // TODO: and suppressions get undone if a program stops unexpectedly (need a way to monitor for this)
                (*suppressions.entry(sender_id).or_insert(0usize)) += 1;
            }

            ResumeNotifications => {
                if let Some(count) = suppressions.get_mut(&sender_id) {
                    // Reduce the suppression count of this program
                    *count -= 1;

                    if *count == 0 {
                        // Remove the program from the list of suppressors
                        suppressions.remove(&sender_id);

                        // TODO: tell the core to notify us when idle
                    }
                }
            }
        }
    }
}
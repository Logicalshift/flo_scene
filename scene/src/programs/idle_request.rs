use crate::*;
use crate::scene_core::*;

use futures::prelude::*;
use futures::future;
use futures::stream;
use futures::channel::mpsc;

use std::collections::{HashMap};

use serde::*;

///
/// ID of the program that sends idle notifications by default
///
pub static IDLE_NOTIFICATION_PROGRAM: StaticSubProgramId = StaticSubProgramId::called("flo_scene::idle_request");

///
/// Idle requests are a way to request a callback that is made when the scene is next idle
///
/// A scene is considered 'idle' when all input streams are waiting with 0 messages remaining, or they 
/// are waiting for the scene to become idle. 
///
/// One use for this is for triggering UI rendering after waiting for a state update to process: this
/// will trigger after all commands have finished processing, which could indicate that the UI is
/// now in a state where it can be rendered without further updates ocurring.
///
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[derive(Serialize, Deserialize)]
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
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct IdleNotification;

impl SceneMessage for IdleRequest {
    fn default_target() -> StreamTarget { (*IDLE_NOTIFICATION_PROGRAM).into() }

    fn allow_thread_stealing_by_default() -> bool { true }
}

impl SceneMessage for IdleNotification {
    fn default_target() -> StreamTarget { StreamTarget::None }
}

///
/// The messages that can be received by the idle program
///
enum IdleProgramMsg {
    Request(SubProgramId, IdleRequest),
    CoreIsIdle(usize)
}

///
/// Runs the idle notification program 
///
pub (crate) async fn idle_subprogram(input_stream: InputStream<IdleRequest>, context: SceneContext) {
    let input_stream                = input_stream.messages_with_sources();
    let mut suppressions            = HashMap::new();
    let mut pending_notifications   = vec![];

    // The core from the context is used to request notifications
    let weak_core                   = context.scene_core();

    // Create the channel used to notify us when the core is idle
    let (send_idle, recv_idle)      = mpsc::channel(1);
    if let Some(core) = weak_core.upgrade() {
        SceneCore::send_idle_notifications_to(&core, send_idle);
    }

    // Merge the notifications (idle notifications and requests)
    let mut input_stream = stream::select(input_stream.map(|(subprogram_id, msg)| IdleProgramMsg::Request(subprogram_id, msg)), recv_idle.map(|count| IdleProgramMsg::CoreIsIdle(count)));

    while let Some(request) = input_stream.next().await {
        use IdleProgramMsg::*;
        use IdleRequest::*;

        match request {
            Request(_, WhenIdle(send_message_to)) => {
                // Add to the list of subprograms to send a message to when an idle request comes in (we'll send multiple notifications if the same program has requested them)

                // Set the core to notify us when idle
                if let Some(core) = weak_core.upgrade() {
                    let idle_count = SceneCore::notify_on_next_idle(&core);

                    pending_notifications.push((idle_count, send_message_to)); 
                }
            },

            Request(sender_id, SuppressNotifications) => {
                // Each program gets its own suppression count, so they can't undo the suppressions of other programs
                // TODO: and suppressions get undone if a program stops unexpectedly (need a way to monitor for this)
                (*suppressions.entry(sender_id).or_insert(0usize)) += 1;
            }

            Request(sender_id, ResumeNotifications) => {
                if let Some(count) = suppressions.get_mut(&sender_id) {
                    // Reduce the suppression count of this program
                    *count -= 1;

                    if *count == 0 {
                        // Remove the program from the list of suppressors
                        suppressions.remove(&sender_id);

                        // Tell the core to notify us when idle
                        if let Some(core) = weak_core.upgrade() {
                            SceneCore::notify_on_next_idle(&core);
                        }
                    }
                }
            },

            CoreIsIdle(idle_count) => {
                // Do nothing if the notifications are suppressed
                if suppressions.is_empty() {
                    // Send any pending idle notification that was queued before we reached idle_count (the value passed in here will be the value returned by the next call to notify_on_next_idle)
                    let mut ready_notifications         = vec![];
                    let mut new_pending_notifications   = vec![];

                    for (expected_count, program_id) in pending_notifications.into_iter() {
                        if expected_count < idle_count {
                            // Anything in the ready_notifications will be sent
                            ready_notifications.push(program_id);
                        } else {
                            // Anything in the 'new' list will keep running
                            // Very rarely, it's possible for an idle notification to be added while a previous request is notifying (we should wait for the next request)
                            new_pending_notifications.push((expected_count, program_id));
                        }
                    }

                    pending_notifications = new_pending_notifications;
                    if !pending_notifications.is_empty() {
                        // Re-request an idle notification if there are any pending requests due to a race condition (this should be very rare)
                        if let Some(core) = weak_core.upgrade() {
                            SceneCore::notify_on_next_idle(&core);
                        }
                    }

                    // Send notifications to everything that's waiting
                    future::join_all(ready_notifications.into_iter()
                        .flat_map(|program_id| context.send(program_id).ok())
                        .map(|mut stream| async move { stream.send(IdleNotification).await.ok(); }))
                        .await;
                }
            }
        }
    }
}
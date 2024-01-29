use crate::*;

use once_cell::sync::{Lazy};

///
/// ID of the program that sends idle notifications by default
///
pub static IDLE_REQUEST_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("IDLE_REQUEST"));

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
    fn default_target() -> StreamTarget { (*IDLE_REQUEST_PROGRAM).into() }
}

impl SceneMessage for IdleNotification {
    fn default_target() -> StreamTarget { StreamTarget::None }
}

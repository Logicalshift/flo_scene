use crate::entity_channel::*;

use std::time::{Duration};

///
/// ID of a timer
///
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimerId(pub usize);

///
/// Message indicating a timeout on a specified timer
///
pub struct Timeout(pub TimerId, pub Duration);

///
/// Requests for the timer entity
///
pub enum TimerRequest {
    /// Fires a single timer event after Duration (from the point this request is retired)
    OneShot(TimerId, Duration, BoxedEntityChannel<'static, Timeout, ()>),

    /// Fires a repeating timer event every Duration (may skip notifications for timeouts that occur while the message is being processed)
    Repeating(TimerId, Duration, BoxedEntityChannel<'static, Timeout, ()>),
}

impl Default for TimerId {
    fn default() -> Self {
        TimerId(usize::default())
    }
}

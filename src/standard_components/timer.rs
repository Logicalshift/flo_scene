use std::time::{Duration};

#[cfg(feature="timer")] use crate::context::*;
#[cfg(feature="timer")] use crate::error::*;
#[cfg(feature="timer")] use crate::entity_channel::*;
#[cfg(feature="timer")] use crate::entity_id::*;

#[cfg(feature="timer")] use futures::prelude::*;
#[cfg(feature="timer")] use futures_timer::{Delay};
#[cfg(feature="timer")] use std::sync::*;
#[cfg(feature="timer")] use std::time::{Instant};

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
    OneShot(TimerId, Duration, BoxedEntityChannel<'static, Timeout>),

    /// Fires a repeating timer event every Duration (may skip notifications for timeouts that occur while the message is being processed)
    Repeating(TimerId, Duration, BoxedEntityChannel<'static, Timeout>),
}

impl Default for TimerId {
    fn default() -> Self {
        TimerId(usize::default())
    }
}

///
/// Creates the timer entity
///
/// This responds to TimerRequests, 
///
#[cfg(feature="timer")]
pub fn create_timer_entity(entity_id: EntityId, context: &Arc<SceneContext>) -> Result<impl EntityChannel<Message=TimerRequest>, CreateEntityError> {
    context.create_entity(entity_id, |context, mut timer_messages| async move {
        while let Some(message) = timer_messages.next().await {
            use TimerRequest::*;

            match message {
                OneShot(timer_id, time, channel) => {
                    context.run_in_background(async move {
                        let mut channel = channel;

                        Delay::new(time).await;
                        channel.send_without_waiting(Timeout(timer_id, time)).await.ok();
                    }).ok();
                }

                Repeating(timer_id, time, channel) => {
                    // 0-delay or very rapid timers are not allowed (we just ignore them)
                    if time <= Duration::from_millis(1) { 
                        continue;
                    }

                    // The timeouts are normalised so that they occur at fixed offsets from the start time
                    let start_time      = Instant::now();
                    let mut next_tick   = time;

                    context.run_in_background(async move { 
                        let mut channel = channel;

                        loop {
                            // Decide how long to wait by using the start time
                            let current_time = Instant::now().duration_since(start_time);

                            while next_tick <= current_time {
                                next_tick += time;
                            }

                            // Wait for the delay to pass (for current_time to reach next_tick)
                            let delay       = next_tick - current_time;
                            let last_tick   = next_tick;

                            Delay::new(delay).await;
                            next_tick += time;

                            // Inform the channel of the timeout
                            let send_result = channel.send_without_waiting(Timeout(timer_id, last_tick)).await;

                            match send_result {
                                Ok(())  => { /* Target responded */ }

                                // Other errors stop the timer
                                Err(_)  => { break; }
                            }
                        }
                    }).ok();
                }
            }
        }
    })
}

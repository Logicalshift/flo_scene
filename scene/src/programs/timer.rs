use crate::*;

use futures::prelude::*;
use futures::{pin_mut};
use futures::future::{poll_fn, BoxFuture};
use futures::task::{Poll, Waker};
use futures_timer::{Delay};
use once_cell::sync::{Lazy};

use std::collections::{VecDeque};
use std::sync::*;
use std::time::{Instant, Duration};

#[cfg(feature="serde_support")] use serde::*;

pub static TIMER_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("TIMER_PROGRAM"));

///
/// The timer program can be used to sent one-off or recurring timer events
///
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub enum TimerRequest {
    /// Sends a `TimeOut` message to a subprogram with the specified ID attached
    CallAfter(SubProgramId, usize, Duration),

    /// Sends a `TimeOut` message to a subprogram with the specified ID attached on a repeating basis
    CallEvery(SubProgramId, usize, Duration),

    /// Cancels all the timer events for a particular subprogram ID
    Cancel(SubProgramId, usize),
}

///
/// A message that is sent when a timer with a particular ID has finished
///
/// The ID is specified in the timer request that caused this message, and the
/// Duration is the true time since the first request was made
///
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TimeOut(pub usize, pub Duration);

impl SceneMessage for TimerRequest {
    fn default_target() -> StreamTarget {
        (*TIMER_PROGRAM).into()
    }
}

impl SceneMessage for TimeOut {}

struct Timer {
    target_program:     SubProgramId,
    timer_id:           usize,
    callback_offset:    Duration,
    repeating:          Option<Duration>,
}

pub fn timer_subprogram(input_stream: InputStream<TimerRequest>, context: SceneContext) -> impl Future<Output=()> {
    // Thread stealing will ensure that timers are started promptly in most cases
    input_stream.allow_thread_stealing(true);

    async move {
        // Record a start time which we can use to keep 'Every' timers running appropriately
        let start_time = Instant::now();

        // The timer_events is an ordered list of the 
        let timer_events                                    = Mutex::new(VecDeque::new());
        let waker: Mutex<Option<Waker>>                     = Mutex::new(None);
        let extra_futures: Mutex<Vec<BoxFuture<'_, ()>>>    = Mutex::new(vec![]);

        // Create a future that monitors the requests and handles timers
        let mut input_stream    = input_stream;
        let request_future      = async {
            let input_stream = &mut input_stream;

            while let Some(next_event) = input_stream.next().await {
                use TimerRequest::*;

                // Measure the time that this timer is being added/started
                let now = Instant::now().duration_since(start_time);

                match next_event {
                    CallAfter(program_id, timer_id, timeout) => {
                        // Add a new timer event
                        timer_events.lock().unwrap().push_back(Timer { 
                            target_program:     program_id,
                            timer_id:           timer_id,
                            callback_offset:    now + timeout,
                            repeating:          None,
                        });
                    },

                    CallEvery(program_id, timer_id, every) => {
                        // Add a new timer event
                        timer_events.lock().unwrap().push_back(Timer { 
                            target_program:     program_id,
                            timer_id:           timer_id,
                            callback_offset:    now + every,
                            repeating:          Some(every),
                        });
                    },

                    Cancel(program_id, timer_id) => {
                        // TODO: this won't cancel repeating events that are in the process of firing

                        // Remove every timer event that matches the program ID/timer ID
                        timer_events.lock().unwrap().retain(|timer| {
                            timer.target_program != program_id || timer.timer_id != timer_id
                        });
                    }
                }

                // Every event changes the timers, so we sort them here (there's no race condition because we don't run the futures in parallel)
                timer_events.lock().unwrap().make_contiguous().sort_by(|a, b| a.callback_offset.cmp(&b.callback_offset));
            }
        };

        // Function that returns a future that waits for a timer to expire
        // We cheat a bit in that we rely on being polled after the requests to update the timer we're tracking when it changes
        let timer_expired = || {
            let mut next_timeout    = None;
            let mut next_timer      = None;
            let timer_events        = &timer_events;

            poll_fn(move |context| {
                let now = Instant::now().duration_since(start_time);

                {
                    let timer_events = timer_events.lock().unwrap();

                    // Get the next time from the timers list
                    if timer_events.is_empty() {
                        // Clear the timer
                        next_timeout    = None;
                        next_timer      = None;
                    } else {
                        let next_callback_time = timer_events[0].callback_offset;

                        // Stop immediately if the timeout in the first timer is expired
                        if now >= next_callback_time {
                            return Poll::Ready(());
                        }

                        // Replace the timer if it doesn't match the current time
                        if next_timeout != Some(next_callback_time) {
                            next_timeout    = Some(next_callback_time);
                            next_timer      = Some(Delay::new(next_callback_time - now));
                        }
                    }
                }

                if let Some(next_timer_future) = next_timer.as_mut() {
                    // Poll the next timer
                    if next_timer_future.poll_unpin(context).is_ready() {
                        // Before we return ready, clear out the variables
                        next_timeout    = None;
                        next_timer      = None;

                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                } else {
                    // No timer, so we'll sleep forever
                    Poll::Pending
                }
            })
        };

        // Create a future for firing the timer events when they expire
        let fire_timer_events = async {
            loop {
                // Wait for the next timer to expire
                timer_expired().await;

                // Fire every timer that has expired. Repeating timers aren't reset until their messages are sent (so they won't build up forever if the target program isn't listening)
                let now                     = Instant::now().duration_since(start_time);
                let mut timer_events_lock   = timer_events.lock().unwrap();

                while let Some(next_event) = timer_events_lock.pop_front() {
                    // Stop once we reach an event that's happening after the current time
                    if next_event.callback_offset > now {
                        timer_events_lock.push_front(next_event);
                        break;
                    }

                    // Fire this event using a future (if the stream isn't available the timer is just cancelled)
                    if let Ok(target_stream) = context.send::<TimeOut>(next_event.target_program) {
                        let timer_events    = &timer_events;
                        let waker           = &waker;

                        extra_futures.lock().unwrap().push(async move {
                            // Send the timeout message
                            let now                 = Instant::now().duration_since(start_time);
                            let mut target_stream   = target_stream;
                            let sent                = target_stream.send(TimeOut(next_event.timer_id, now - next_event.callback_offset)).await.is_ok();

                            // Requeue the timer if it's repeating (must have sent correctly, and be a non-zero repeat time)
                            if sent {
                                // TODO: also cancel the repeating event if 'Cancel' was called while we were sending the event
                                if let Some(repeat_duration) = next_event.repeating {
                                    if repeat_duration > Duration::ZERO {
                                        // Decide when the next event should fire
                                        let now             = Instant::now().duration_since(start_time);
                                        let mut next_offset = next_event.callback_offset;

                                        while next_offset < now { next_offset += repeat_duration; }

                                        // Queue a new event for when this is repeated
                                        let mut timer_events_lock = timer_events.lock().unwrap();

                                        timer_events_lock.push_back(Timer {
                                            target_program:     next_event.target_program,
                                            timer_id:           next_event.timer_id,
                                            callback_offset:    next_offset,
                                            repeating:          next_event.repeating,
                                        });
                                        timer_events_lock.make_contiguous().sort_by(|a, b| a.callback_offset.cmp(&b.callback_offset));

                                        // Reawaken the polling loop to reschedule the timer
                                        let waker = waker.lock().unwrap().take();
                                        if let Some(waker) = waker { waker.wake(); }
                                    }
                                }
                            }
                        }.boxed());
                    }
                }
            }
        };

        // Poll the various futures we need to track using a manual poll_fn
        pin_mut!(request_future);
        pin_mut!(fire_timer_events);

        poll_fn(|context| {
            *waker.lock().unwrap() = Some(context.waker().clone());

            // Poll the future request stream: if it finishes, the whole set of timers is finished
            if request_future.as_mut().poll(context).is_ready() {
                return Poll::Ready(());
            }

            // Poll for timers timing out
            if fire_timer_events.as_mut().poll(context).is_ready() {
                return Poll::Ready(());
            }

            // Poll the message senders, if there are any
            extra_futures.lock().unwrap().retain_mut(|future| {
                future.poll_unpin(context).is_pending()
            });

            Poll::Pending
        }).await;
    }
}
use crate::*;

use futures::prelude::*;
use futures::{pin_mut};
use futures::future::{poll_fn};
use futures::task::{Poll};
use once_cell::sync::{Lazy};

use std::sync::*;
use std::time::{Instant, Duration};

pub static TIMER_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("TIMER_PROGRAM"));

///
/// The timer program can be used to sent one-off or recurring timer events
///
#[derive(Copy, Clone, Debug)]
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
    repeating:          bool,
}

pub fn timer_subprogram(input_stream: InputStream<TimerRequest>, context: SceneContext) -> impl Future<Output=()> {
    // Thread stealing will ensure that timers are started promptly in most cases
    input_stream.allow_thread_stealing(true);

    async move {
        // Record a start time which we can use to keep 'Every' timers running appropriately
        let start_time = Instant::now();

        // The timer_events is an ordered list of the 
        //let timer_events = Mutex::new(vec![]);

        // Create a future that monitors the requests and handles timers
        let mut input_stream    = input_stream;
        let request_future      = async {
            let input_stream = &mut input_stream;

            while let Some(next_event) = input_stream.next().await {
                use TimerRequest::*;

                match next_event {
                    CallAfter(program_id, timer_id, timeout) => {
                        todo!()
                    },

                    CallEvery(program_id, timer_id, every) => {
                        todo!()
                    },

                    Cancel(program_id, timer_id) => {
                        todo!()
                    }
                }
            }
        };

        // Poll the various futures we need to track using a manual poll_fn
        pin_mut!(request_future);

        poll_fn(|context| {
            // Poll the future request stream: if it finishes, the whole set of timers is finished
            if request_future.as_mut().poll(context).is_ready() {
                return Poll::Ready(());
            }

            // TODO: Poll the next timeout timer, if there is one

            // TODO: Poll the message senders, if there are any

            Poll::Pending
        }).await;
    }
}
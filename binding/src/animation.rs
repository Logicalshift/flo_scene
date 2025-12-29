use flo_binding::*;
use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::time::{Duration};

///
/// Describes an animation function
///
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationDescription {
    /// Animation that runs linearly for the specified amount of time, in seconds
    Linear(f64),

    // An 'ease in' animation (which speeds up until the final position is reached)
    EaseIn(f64),

    /// An 'ease out' animation (which slows down until the final position is reached)
    EaseOut(f64),

    /// Eases in for the first half of the animation, then eases out
    EaseInOut(f64),
}

impl AnimationDescription {
    ///
    /// Returns a function that converts time in seconds to the 0-1 animation range
    ///
    pub fn transform_fn(&self) -> Box<dyn Send + Sync + Fn(f64) -> f64> {
        use AnimationDescription::*;

        match *self {
            Linear(time)    => Box::new(move |seconds| Self::linear(seconds, time)),
            EaseIn(time)    => Box::new(move |seconds| Self::ease_in(Self::linear(seconds, time))),
            EaseOut(time)   => Box::new(move |seconds| Self::ease_out(Self::linear(seconds, time))),
            EaseInOut(time) => Box::new(move |seconds| Self::ease_in_out(Self::linear(seconds, time))),
        }
    }

    ///
    /// Returns the duration of this animation in seconds
    ///
    pub fn duration_seconds(&self) -> f64 {
        use AnimationDescription::*;

        match *self {
            Linear(time)    => time,
            EaseIn(time)    => time,
            EaseOut(time)   => time,
            EaseInOut(time) => time,
        }
    }

    /// Converts a time in seconds to a linear animation time
    #[inline]
    fn linear(seconds: f64, total_time: f64) -> f64 {
        (seconds/total_time).clamp(0.0, 1.0)
    }

    /// Converts a linear time to an 'ease-in' time
    #[inline]
    fn ease_in(t: f64) -> f64 {
        (t.clamp(0.0, 1.0)).powi(3)
    }

    /// Converts a linear time to an 'ease-out' time
    #[inline]
    fn ease_out(t: f64) -> f64 {
        1.0 - Self::ease_in(1.0 - t)
    }

    /// Converts a linear time to an 'ease-in-out' time
    #[inline]
    fn ease_in_out(t: f64) -> f64 {
        if t < 0.5 {
            0.5 * Self::ease_in(t * 2.0)
        } else {
            0.5 + 0.5 * Self::ease_out((t-0.5) * 2.0)
        }
    }

    ///
    /// Returns a subprogram that will implement this animation when run
    ///
    pub fn program(&self, t: Binding<f64>, interval: f64) -> impl 'static + Send + Sync + FnOnce(InputStream<TimeOut>, SceneContext) -> BoxFuture<'static, ()> {
        // Fetch state of this animation
        let transform_fn        = self.transform_fn();
        let duration_seconds    = self.duration_seconds();
        let interval            = Duration::from_nanos((interval * 1_000_000_000.0) as _);

        move |input, context| {
            let our_program_id = context.current_program_id().unwrap();

            // Time is initially reset to 0
            t.set(0.0);

            async move {
                // Request timeout events
                context.send_message(TimerRequest::CallEvery(our_program_id, 0, interval)).await.ok();

                let mut input = input;
                while let Some(timeout) = input.next().await {
                    // Get the curren time
                    let seconds = (timeout.1.as_nanos() as f64) / 1_000_000_000.0;

                    // Update the 't' value
                    t.set(transform_fn(seconds));

                    // Stop once the end of the animation is reached
                    if seconds >= duration_seconds {
                        break;
                    }
                }

                // Ensure the timer is stopped before we exit
                context.send_message(TimerRequest::Cancel(our_program_id, 0)).await.ok();
            }.boxed()
        }
    }
}

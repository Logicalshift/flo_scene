use super::binding_program::*;

use flo_binding::*;
use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::time::{Duration};

///
/// Describes an animation function
///
pub struct AnimationDescription {
    /// The animation to perform
    animation_type: AnimationType,

    /// Action to take once the animation has finished
    when_finished: Option<Box<dyn Send + Sync + FnOnce(SceneContext) -> BoxFuture<'static, ()>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum AnimationType {
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
    /// Creates a linear animation that moves at one speed
    ///
    pub fn linear(duration_seconds: f64) -> Self {
        AnimationDescription { 
            animation_type: AnimationType::Linear(duration_seconds),
            when_finished:  None,
        }
    }

    ///
    /// Creates an ease-in animation that speeds up towards the end
    ///
    pub fn ease_in(duration_seconds: f64) -> Self {
        AnimationDescription { 
            animation_type: AnimationType::EaseIn(duration_seconds),
            when_finished:  None,
        }
    }

    ///
    /// Creates an ease-out animation that slows down towards the end
    ///
    pub fn ease_out(duration_seconds: f64) -> Self {
        AnimationDescription { 
            animation_type: AnimationType::EaseOut(duration_seconds),
            when_finished:  None,
        }
    }

    ///
    /// Creates an ease-out animation that speeds up then slows down
    ///
    pub fn ease_in_out(duration_seconds: f64) -> Self {
        AnimationDescription { 
            animation_type: AnimationType::EaseInOut(duration_seconds),
            when_finished:  None,
        }
    }

    ///
    /// Adds an action to perform when the animation is completed
    ///
    pub fn with_when_finished<TFuture>(mut self, when_finished: impl 'static + Send + Sync + FnOnce(SceneContext) -> TFuture) -> Self 
    where 
        TFuture: 'static + Send + Future<Output=()>,
    {
        self.when_finished = Some(Box::new(move |context| when_finished(context).boxed()));

        self
    }

    ///
    /// Returns a function that converts time in seconds to the 0-1 animation range
    ///
    pub fn transform_fn(&self) -> Box<dyn Send + Sync + Fn(f64) -> f64> {
        use AnimationType::*;

        match self.animation_type {
            Linear(time)    => Box::new(move |seconds| Self::linear_t(seconds, time)),
            EaseIn(time)    => Box::new(move |seconds| Self::ease_in_t(Self::linear_t(seconds, time))),
            EaseOut(time)   => Box::new(move |seconds| Self::ease_out_t(Self::linear_t(seconds, time))),
            EaseInOut(time) => Box::new(move |seconds| Self::ease_in_out_t(Self::linear_t(seconds, time))),
        }
    }

    ///
    /// Returns the duration of this animation in seconds
    ///
    pub fn duration_seconds(&self) -> f64 {
        use AnimationType::*;

        match self.animation_type {
            Linear(time)    => time,
            EaseIn(time)    => time,
            EaseOut(time)   => time,
            EaseInOut(time) => time,
        }
    }

    /// Converts a time in seconds to a linear animation time
    #[inline]
    fn linear_t(seconds: f64, total_time: f64) -> f64 {
        (seconds/total_time).clamp(0.0, 1.0)
    }

    /// Converts a linear time to an 'ease-in' time
    #[inline]
    fn ease_in_t(t: f64) -> f64 {
        (t.clamp(0.0, 1.0)).powi(3)
    }

    /// Converts a linear time to an 'ease-out' time
    #[inline]
    fn ease_out_t(t: f64) -> f64 {
        1.0 - Self::ease_in_t(1.0 - t)
    }

    /// Converts a linear time to an 'ease-in-out' time
    #[inline]
    fn ease_in_out_t(t: f64) -> f64 {
        if t < 0.5 {
            0.5 * Self::ease_in_t(t * 2.0)
        } else {
            0.5 + 0.5 * Self::ease_out_t((t-0.5) * 2.0)
        }
    }

    ///
    /// Returns a subprogram that will implement this animation when run
    ///
    /// This program updates the specified binding every tick of the animation, so it's not very useful by itself. Use the `run_animation`
    /// function to set up a more full-featured animation subprogram.
    ///
    pub fn program(mut self, t: Binding<f64>, interval: f64) -> impl 'static + Send + Sync + FnOnce(InputStream<TimeOut>, SceneContext) -> BoxFuture<'static, ()> {
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

                let mut input = input.ready_chunks(100);
                while let Some(timeout) = input.next().await {
                    let Some(timeout) = timeout.last() else { continue; };

                    // Get the current time
                    let seconds = (timeout.1.as_nanos() as f64) / 1_000_000_000.0;

                    // Update the 't' value
                    t.set(transform_fn(seconds));

                    // Stop once the end of the animation is reached
                    if seconds >= duration_seconds {
                        // Wait for the scene to become idle a couple of times so the last 't' value can be processed before shutting down
                        context.wait_for_idle(0).await;
                        break;
                    }
                }

                // Ensure the timer is stopped before we exit
                context.send_message(TimerRequest::Cancel(our_program_id, 0)).await.ok();

                if let Some(when_finished) = self.when_finished.take() {
                    when_finished(context.clone()).await;
                }
            }.boxed()
        }
    }

    ///
    /// Takes the 'when finished' function in order to run it
    ///
    pub (crate) fn take_when_finished(&mut self) -> Option<Box<dyn Send + Sync + FnOnce(SceneContext) -> BoxFuture<'static, ()>>> {
        self.when_finished.take()
    }
}

impl Default for AnimationDescription {
    #[inline]
    fn default() -> Self {
        Self::linear(1.0)
    }
}


///
/// Runs an animation binding in the specified scene context, returning the subprogram ID assigned to the running animation program
///
/// The parameter to `binding` is the `t` value for the animation, and the `action` is the action to perform every time the animation
/// value changes (eg to redraw the thing being animated). The subprogram can be stopped to end the animation early. The 'interval' is
/// how often the animation runs.
///
/// For example:
///
/// ```
/// # use flo_scene::*;
/// # use flo_scene_binding::*;
/// # use flo_binding::*;
/// # use futures::prelude::*;
/// # use serde::*;
/// # let scene         = Scene::default();
/// # let program_id    = SubProgramId::new();
/// # let binding       = bind(42);
/// #[derive(Serialize, Deserialize)]
/// enum DrawMessage { DrawAt(f64, f64) }
/// impl SceneMessage for DrawMessage { }
///
/// let action = BindingAction::new(|value: (f64, f64), context| async move { context.send_message(DrawMessage::DrawAt(value.0, value.1)).await.ok(); });
/// scene.add_subprogram(program_id, move |input: InputStream<()>, context| async move {
///     run_animation(&context, AnimationDescription::ease_in_out(1.0), 1.0/60.0, |t| computed(move || (t.get().sin(), t.get().cos())).into(), action).await;
///     
///     let mut input = input;
///     while let Some(_) = input.next().await { }
/// }, 20);
/// ```
///
/// This function provides a generic interface that makes very few assumptions about how the animation should work. It will often make sense to wrap
/// this in your own function to deal with specific cases (eg, where you already know the interface, or the binding action is commonly known)
///
pub async fn run_animation<TValue, TFn, TFuture>(context: &SceneContext, animation: AnimationDescription, interval: f64, binding: impl FnOnce(BindRef<f64>) -> BindRef<TValue>, action: impl Into<BindingAction<TValue, TFn, TFuture>>) -> SubProgramId
where
    TFn:        'static + Send + FnMut(TValue, SceneContext) -> TFuture,
    TFuture:    'static + Send + Future<Output=()>,
    TValue:     'static + Send,
{
    // TODO: two subprograms to generate each animation action is slightly inefficient. It might be better to combine them (though this is more complex, and it isn't clear if the extra overhead will ever matter)

    // We run two subprograms to run the animation: the animation program updates the binding, and the binding program performs actions based on when that changes
    let animation_program_id    = SubProgramId::new();
    let binding_program_id      = SubProgramId::new();

    // Set up the animation binding
    let t                   = bind(0.0);
    let animation_program   = animation.program(t.clone(), interval);
    let binding             = binding(t.into());

    // Start the animation program as a child of the current prorgram
    let parent_program_id = context.current_program_id().unwrap();

    let Ok(_) = context.send_message(SceneControl::start_child_program(animation_program_id, parent_program_id, animation_program, 20)).await else { return animation_program_id; };

    // Start the binding program as a child of the animation program
    let action = action.into();
    context.send_message(SceneControl::start_child_program(binding_program_id, animation_program_id, move |input, context| binding_program(input, context, binding, action), 1)).await.ok();

    // Return result is the animation program ID (binding program runs a child program of this)
    animation_program_id
}

///
/// Runs a subprogram that will animate the specified binding between 0.0-1.0 according to the supplied animation description
///
/// The interval is the time in seconds between updates of the binding. The return value is the subprogram ID, which can be used to cancel this animation.
///
pub async fn run_binding_animation(context: &SceneContext, animation: AnimationDescription, interval: f64, binding: Binding<f64>) -> SubProgramId {
    // This just starts the animation program
    let animation_program_id    = SubProgramId::new();
    let animation_program       = animation.program(binding, interval);
    let parent_program_id       = context.current_program_id().unwrap();

    context.send_message(SceneControl::start_child_program(animation_program_id, parent_program_id, move |input, context| animation_program(input, context), 20)).await.ok();

    animation_program_id
}

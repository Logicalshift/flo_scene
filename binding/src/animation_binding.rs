use super::animation::*;

use flo_binding::*;
use flo_binding::releasable::*;
use flo_scene::*;
use flo_scene::programs::*;
use futures::prelude::*;
use serde::*;
use serde::de::{Error as DeError};
use serde::ser::{Error as SeError};

use std::collections::{HashSet};
use std::sync::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, Duration};

static NEXT_IDENTIFIER: AtomicUsize                     = AtomicUsize::new(0);
static ANIMATION_BINDING_SUBPROGRAM: StaticSubProgramId = StaticSubProgramId::called("flo_scene_binding::animation_binding");

///
/// The shared core of an animation binding
///
pub (crate) struct AnimationBindingCore {
    /// Unique identifier for this core (used to ensure that we don't cause the same animation to run more than once)
    identifier: usize,

    /// Actions to perform when the value is changed
    when_changed: Vec<ReleasableNotifiable>,

    /// The instant when this animation was started
    start_time: Option<Instant>,

    /// The description of the animation that should be performed by this binding
    description: Box<dyn Send + Sync + Fn(f64) -> f64>,

    /// The scene program which updates this message
    target: OutputSink<AnimationBindingMessage>,

    /// The value of this animation
    value: f64,
}

///
/// An animation binding moves between 0 and 1, updating every 1/60th of a second.
///
#[derive(Clone)]
pub struct AnimationBinding {
    /// Shared core
    core: Arc<Mutex<AnimationBindingCore>>,
}

///
/// Messages sent to the animation binding program in the scene
///
pub (crate) enum AnimationBindingMessage {
    /// 1/60th of a second has passed
    Tick(Duration, Instant),

    /// Sets the time in seconds between ticks (defaults to 1/60th of a second)
    SetTickRate(f64),

    /// Tracks and updates an animation binding
    AddAnimationBinding(Weak<Mutex<AnimationBindingCore>>),
}

impl SceneMessage for AnimationBindingMessage {
    fn default_target() -> StreamTarget { (*ANIMATION_BINDING_SUBPROGRAM).into() }

    fn serializable() -> bool { false }

    fn initialise(init_context: &impl SceneInitialisationContext) {
        init_context.add_subprogram(*ANIMATION_BINDING_SUBPROGRAM, animation_binding_program, 100);
        init_context.connect_programs((), StreamTarget::Filtered(FilterHandle::for_filter(|msgs| msgs.map(|msg: TimeOut| AnimationBindingMessage::Tick(msg.1, Instant::now()))), *ANIMATION_BINDING_SUBPROGRAM), StreamId::with_message_type::<TimeOut>()).unwrap();
    }
}

impl Serialize for AnimationBindingMessage {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        Err(S::Error::custom("AnimationBindingMessage cannot be serialized"))
    }
}

impl<'a> Deserialize<'a> for AnimationBindingMessage {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a> 
    {
        Err(D::Error::custom("AnimationBindingMessage cannot be serialized"))
    }
}

impl AnimationBinding {
    ///
    /// The time when this animation was started, or None if it's not running
    ///
    pub fn start_time(&self) -> Option<Instant> {
        self.core.lock().unwrap().start_time
    }

    ///
    /// Starts the animation associated with this binding running
    ///
    pub fn start(&self) {
        // Start time is the immediate time before locking
        let start_time = Instant::now();

        let mut core = self.core.lock().unwrap();

        // If the animation is already running, then don't start it again (if we want to restart an animation, need to call stop then start)
        if core.start_time.is_some() {
            return;
        }

        // Update the start time of the core
        core.start_time = Some(start_time);

        // Tell the core to start running this animation
        core.target.send_immediate(AnimationBindingMessage::AddAnimationBinding(Arc::downgrade(&self.core))).ok();
    }

    ///
    /// Stops this animation from running
    ///
    pub fn stop(&self) {
        self.core.lock().unwrap().start_time = None;
    }

    ///
    /// Updates the animation performed by this binding
    ///
    pub fn change_animation(&self, description: AnimationDescription) {
        self.core.lock().unwrap().description = description.transform_fn();
    }

    ///
    /// If there are any notifiables in this object that aren't in use, remove them
    ///
    pub fn filter_unused_notifications(&self) {
        self.core.lock().unwrap().when_changed.retain(|releasable| releasable.is_in_use());
    }

    ///
    /// Changes the tick rate of the animations running in the scene
    ///
    pub async fn set_tick_rate(new_tick_rate: f64, context: &SceneContext) {
        context.send_message(AnimationBindingMessage::SetTickRate(new_tick_rate)).await.ok();
    }
}

impl Changeable for AnimationBinding {
    fn when_changed(&self, what: Arc<dyn Notifiable>) -> Box<dyn Releasable> {
        let releasable = ReleasableNotifiable::new(what);
        self.core.lock().unwrap().when_changed.push(releasable.clone_as_owned());

        self.filter_unused_notifications();

        Box::new(releasable)
    }
}

impl Bound for AnimationBinding {
    type Value = f64;

    fn get(&self) -> Self::Value {
        self.core.lock().unwrap().value
    }

    fn watch(&self, what: Arc<dyn Notifiable>) -> Arc<dyn Watcher<Self::Value>> {
        let watch_binding           = self.clone();
        let (watcher, notifiable)   = NotifyWatcher::new(move || watch_binding.get(), what);

        self.core.lock().unwrap().when_changed.push(notifiable);
        self.filter_unused_notifications();

        Arc::new(watcher)
    }
}

///
/// The animation binding program is a singleton program that runs in the scene that updates all of the active animation bindings according to the timer
///
async fn animation_binding_program(input: InputStream<AnimationBindingMessage>, context: SceneContext) {
    let program_id  = context.current_program_id().unwrap();
    let mut timer   = context.send::<TimerRequest>(()).unwrap();

    // State of the animations
    let mut tick_time       = 1.0/60.0;
    let mut is_ticking      = false;
    let mut cores           = vec![];
    let mut active_cores    = HashSet::new();

    // Run the main loop
    let mut input = input;
    while let Some(msg) = input.next().await {
        match msg {
            AnimationBindingMessage::AddAnimationBinding(binding_core) => {
                if let Some(core) = binding_core.upgrade() {
                    let identifier = core.lock().unwrap().identifier;

                    if !active_cores.contains(&identifier) {
                        // Add this core to the set that we're monitoring
                        active_cores.insert(identifier);
                        cores.push((identifier, binding_core));

                        // Start the timer if necessary
                        if !is_ticking {
                            let duration = Duration::from_nanos((1_000_000_000.0 * tick_time) as _);
                            timer.send(TimerRequest::CallEvery(program_id, 0, duration)).await.ok();

                            is_ticking = true;
                        }
                    }
                }
            },

            AnimationBindingMessage::SetTickRate(new_tick_time) => {
                // Change the tick time
                tick_time = new_tick_time;

                // Re-request the ticks if the timer is running
                if is_ticking {
                    let duration = Duration::from_nanos((1_000_000_000.0 * tick_time) as _);
                    timer.send(TimerRequest::Cancel(program_id, 0)).await.ok();
                    timer.send(TimerRequest::CallEvery(program_id, 0, duration)).await.ok();
                }
            },

            AnimationBindingMessage::Tick(_duration, now) => {
                // Update the cores
                let mut finished_cores  = vec![];
                let mut to_notify       = vec![];
                for (idx, (identifier, core)) in cores.iter().enumerate() {
                    // Upgrade the core so we can update it
                    let identifier          = *identifier;
                    let Some(core)          = core.upgrade() else { finished_cores.push((idx, identifier)); continue; };
                    let mut core            = core.lock().unwrap();

                    // If there's a start time, this animation is running (else it's stopped)
                    let Some(start_time)    = core.start_time else { finished_cores.push((idx, identifier)); continue; };

                    // Update the value
                    let since_start         = now.duration_since(start_time);
                    let seconds_since_start = (since_start.as_nanos() as f64) / 1_000_000_000.0;
                    let new_value           = (core.description)(seconds_since_start);

                    if new_value == core.value { continue; }
                    if new_value >= 1.0 {
                        // Animation stops once the value hits 1.0
                        finished_cores.push((idx, identifier));
                    }

                    core.value = new_value;

                    // Notify the bindings
                    to_notify.extend(core.when_changed.iter().map(|notify| notify.clone_for_inspection()));
                }

                // Remove any cores that are finished
                for (finished_idx, identifier) in finished_cores.into_iter().rev() {
                    cores.remove(finished_idx);
                    active_cores.remove(&identifier);
                }

                // Notify the updated bindings
                for notify in to_notify {
                    notify.mark_as_changed();
                }

                // Stop ticking when we run out of cores
                if cores.is_empty() {
                    timer.send(TimerRequest::Cancel(program_id, 0)).await.ok();
                    is_ticking = false;
                }
            },
        }
    }
}

///
/// Creates a stopped animation binding in the specified scene context
///
pub fn animate_binding(description: AnimationDescription, context: &SceneContext) -> AnimationBinding {
    let identifier  = NEXT_IDENTIFIER.fetch_add(1, Ordering::Relaxed);
    let core        = AnimationBindingCore {
        identifier:     identifier,
        when_changed:   vec![],
        start_time:     None,
        description:    description.transform_fn(),
        target:         context.send::<AnimationBindingMessage>(()).unwrap(),
        value:          0.0,
    };
    let core        = Arc::new(Mutex::new(core));

    AnimationBinding {
        core
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn update_animation_binding() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        pub struct Msg(f64);

        impl SceneMessage for Msg { }

        let scene               = Scene::default();
        let test_program        = SubProgramId::called("test::test");
        let animation_program   = SubProgramId::called("test::animation_program");

        // Disable the timer by dumping the messages
        scene.connect_programs((), StreamTarget::None, StreamId::with_message_type::<TimerRequest>()).unwrap();

        scene.add_subprogram(animation_program, |_input: InputStream<()>, context: SceneContext| async move {
            // Create our animation binding and start it
            let animation_binding = animate_binding(AnimationDescription::linear(1.0), &context);
            animation_binding.start();

            // Need to know the start time to send updates to the animation program
            let start_time = animation_binding.start_time().unwrap();

            // Follow updates to the animation value
            let mut updates = follow(animation_binding);

            let tenth   = start_time + Duration::from_millis(100);
            let half    = start_time + Duration::from_millis(500);
            let full    = start_time + Duration::from_millis(1000);

            // Timer events should generate messages from the follow
            let mut animation = context.send::<AnimationBindingMessage>(()).unwrap();

            let val = updates.next().await.unwrap();
            context.send_message(Msg(val)).await.unwrap();

            animation.send(AnimationBindingMessage::Tick(Duration::from_millis(100), tenth)).await.ok();
            let val = updates.next().await.unwrap();
            context.send_message(Msg(val)).await.unwrap();

            animation.send(AnimationBindingMessage::Tick(Duration::from_millis(500), half)).await.ok();
            let val = updates.next().await.unwrap();
            context.send_message(Msg(val)).await.unwrap();

            animation.send(AnimationBindingMessage::Tick(Duration::from_millis(1000), full)).await.ok();
            let val = updates.next().await.unwrap();
            context.send_message(Msg(val)).await.unwrap();
        }, 1);

        // Run in the test harness
        TestBuilder::new()
            .expect_message_matching(Msg(0.0), "Zero")
            .expect_message_matching(Msg(0.1), "Tenth")
            .expect_message_matching(Msg(0.5), "Half")
            .expect_message_matching(Msg(1.0), "Full")
            .run_in_scene(&scene, test_program);
    }
}

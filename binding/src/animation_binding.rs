use super::animation::*;

use flo_binding::*;
use flo_scene::*;
use serde::*;
use serde::de::{Error as DeError};
use serde::ser::{Error as SeError};

use std::sync::*;
use std::time::{Instant};

///
/// The shared core of an animation binding
///
pub (crate) struct AnimationBindingCore {
    /// Unique identifier for this core (used to ensure that we don't cause the same animation to run more than once)
    identifier: usize,

    /// The instant when this animation was started
    start_time: Option<Instant>,

    /// The description of the animation that should be performed by this binding
    description: AnimationDescription,

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
    Tick,

    /// Tracks and updates an animation binding
    AddAnimationBinding(Weak<Mutex<AnimationBindingCore>>),
}

impl SceneMessage for AnimationBindingMessage {
    fn serializable() -> bool { false }
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
}

impl Changeable for AnimationBinding {
    fn when_changed(&self, what: Arc<dyn Notifiable>) -> Box<dyn Releasable> {
        todo!()
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

        // self.value.lock().unwrap().when_changed.push(notifiable);
        // self.value.lock().unwrap().filter_unused_notifications();

        Arc::new(watcher)
    }
}

///
/// Creates a stopped animation binding in the specified scene context
///
pub fn animate_binding(description: AnimationDescription, context: &SceneContext) -> AnimationBinding {
    todo!()    
}

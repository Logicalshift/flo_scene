use futures::task;

use std::sync::*;

///
/// Shared core for a scene waker
///
struct SceneWakerCore {
    /// True if the future associated with this waker is awake
    is_awake: bool,

    /// If we haven't awoken the scene, and it's waiting for us, this will wake it up
    scene_waker: Option<task::Waker>,
}

///
/// Waker for a future in a scene
///
pub struct SceneWaker {
    /// Shared core
    core: Arc<Mutex<SceneWakerCore>>
}

impl SceneWaker {
    ///
    /// Creates a scene waker from a future context
    ///
    pub fn from_context(context: &task::Context) -> SceneWaker {
        let waker   = context.waker().clone();
        let core    = SceneWakerCore {
            is_awake:       true,
            scene_waker:    Some(waker),
        };

        SceneWaker {
            core: Arc::new(Mutex::new(core))
        }
    }

    ///
    /// True if this core has been awoken since it was last polled
    ///
    pub fn is_awake(&self) -> bool {
        self.core.lock().unwrap().is_awake
    }

    ///
    /// Puts this waker back to sleep (generally called *before* polling the associated future)
    ///
    pub fn go_to_sleep(&self, context: &task::Context) {
        let mut core        = self.core.lock().unwrap();

        core.is_awake       = false;
        core.scene_waker    = Some(context.waker().clone());
    }
}

impl task::ArcWake for SceneWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let waker = {
            // Mark the core as awake
            let mut core    = arc_self.core.lock().unwrap();
            core.is_awake   = true;

            // Take the waker from the core
            core.scene_waker.take()
        };

        // Trigger the waker outside of the core lock
        if let Some(waker) = waker {
            waker.wake()
        }
    }
}

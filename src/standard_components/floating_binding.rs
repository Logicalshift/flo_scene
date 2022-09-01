use flo_binding::*;
use flo_binding::releasable::*;
use flo_binding::binding_context::*;

use std::sync::*;

///
/// A floating binding is associated with a binding target that will be bound at some unspecified point in the future
///
/// The design of this structure allows the binding to be used directly, or as a waypoint for retrieving the final
/// `BindRef` either as a future or by blocking. In single-threaded contexts, the version using a future is much to
/// be preferred.
///
pub struct FloatingBinding<TValue> {
    /// The core of this binding (shared with the target)
    core: Arc<Mutex<FloatingBindingCore<TValue>>>,
}

///
/// Core shared between a floating binding and its target
///
struct FloatingBindingCore<TValue> {
    /// The binding, once it has been supplied by the remote object
    binding: FloatingState<BindRef<TValue>>,

    /// If there are any notifications in 'when_bound', this will pass on the notification
    binding_watcher: Option<Box<dyn Releasable>>,

    /// The actions to notify when the binding is updated
    when_bound: Vec<ReleasableNotifiable>,
}

///
/// The possible state of a 'floating' binding value
///
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatingState<TValue> {
    /// The binding has not been set by the target yet
    Waiting,

    /// The target never bound the value before it was dropped
    Abandoned,

    /// The value was not available from the target
    Missing,

    /// The value was retrieved from the target
    Value(TValue),
}

impl<TValue> FloatingState<TValue> {
    ///
    /// Maps the value contained within this floating state
    ///
    pub fn map<TMapValue>(self, map_fn: impl FnOnce(TValue) -> TMapValue) -> FloatingState<TMapValue> {
        match self {
            FloatingState::Waiting      => FloatingState::Waiting,
            FloatingState::Abandoned    => FloatingState::Abandoned,
            FloatingState::Missing      => FloatingState::Missing,
            FloatingState::Value(value) => FloatingState::Value(map_fn(value)),
        }
    }

    ///
    /// Unwraps the value inside this floating state
    ///
    pub fn unwrap(self) -> TValue {
        match self {
            FloatingState::Waiting      => panic!("called `FloatingState::unwrap()` on a `Waiting` value"),
            FloatingState::Abandoned    => panic!("called `FloatingState::unwrap()` on an `Abandoned` value"),
            FloatingState::Missing      => panic!("called `FloatingState::unwrap()` on a `Missing` value"),
            FloatingState::Value(value) => value,
        }
    }
}

impl<TValue> Into<Option<TValue>> for FloatingState<TValue> {
    fn into(self) -> Option<TValue> {
        match self {
            FloatingState::Waiting      => None,
            FloatingState::Abandoned    => None,
            FloatingState::Missing      => None,
            FloatingState::Value(value) => Some(value),
        }
    }
}

impl<TValue> Clone for FloatingBinding<TValue> {
    fn clone(&self) -> FloatingBinding<TValue> {
        FloatingBinding {
            core: Arc::clone(&self.core),
        }
    }
}

impl<TValue> Changeable for FloatingBinding<TValue> {
    fn when_changed(&self, what: Arc<dyn Notifiable>) -> Box<dyn Releasable> {
        let mut core = self.core.lock().unwrap();

        // We notify via the core if the binding has not been set, or the binding itself if it has
        match &core.binding {
            FloatingState::Waiting      => {
                // The remote value hasn't arrived yet: notify via this object
                let releasable = ReleasableNotifiable::new(what);
                core.when_bound.push(releasable.clone_as_owned());

                Box::new(releasable)
            },

            FloatingState::Value(value) => {
                // Remote value is available: notify from here
                value.when_changed(what)
            }

            _                           => { 
                // Error state: we won't notify at all
                Box::new(ReleasableNotifiable::new(what))
            }
        }
    }
}

impl<TValue> Bound for FloatingBinding<TValue> 
where
    TValue: 'static,
{
    type Value = FloatingState<TValue>;

    #[inline]
    fn get(&self) -> Self::Value {
        // Fetch the value contained within the core (don't keep the lock in case of re-entrancy)
        let core_value = { self.core.lock().unwrap().binding.clone() };

        // Fetch the contents of the bindref
        match core_value {
            FloatingState::Abandoned    => FloatingState::Abandoned,
            FloatingState::Missing      => FloatingState::Missing,
            FloatingState::Value(value) => FloatingState::Value(value.get()),

            FloatingState::Waiting      => {
                // Only add this to the binding context if the value can change and isn't a BindRef yet (if the bindref has been supplied, then it becomes the dependency)
                BindingContext::add_dependency(self.clone());
                FloatingState::Waiting
            },
        }
    }

    #[inline]
    fn watch(&self, what: Arc<dyn Notifiable>) -> Arc<dyn Watcher<Self::Value>> {
        let mut core                = self.core.lock().unwrap();
        let watch_binding           = self.clone();
        let (watcher, notifiable)   = NotifyWatcher::new(move || watch_binding.get(), what);

        core.when_bound.push(notifiable);

        Arc::new(watcher)
    }
}

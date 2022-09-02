use crate::error::*;

use flo_binding::*;
use flo_binding::releasable::*;
use flo_binding::binding_context::*;

use futures::prelude::*;
use futures::channel::mpsc;

use std::mem;
use std::sync::*;

///
/// A floating binding is associated with a binding target that will be bound to its true value at some unspecified point in the future
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
/// The floating binding target is used to supply the binding when it becomes available
///
pub struct FloatingBindingTarget<TValue> {
    /// The core of this binding (shared with the target)
    core: Arc<Mutex<FloatingBindingCore<TValue>>>,
}

///
/// Core shared between a floating binding and its target
///
struct FloatingBindingCore<TValue> {
    /// The binding, once it has been supplied by the remote object
    binding: FloatingState<BindRef<TValue>>,

    /// If there are any notifications in 'when_changed', this will pass on the notification
    binding_watcher: Option<Box<dyn Releasable>>,

    /// The actions to notify when the binding is updated
    when_changed: Vec<ReleasableNotifiable>,
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
                core.when_changed.push(releasable.clone_as_owned());

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
    TValue: 'static + Clone + Send,
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
        match &core.binding {
            FloatingState::Waiting => {
                // Create a watcher that notifies when this changes
                let watch_binding           = self.clone();
                let (watcher, notifiable)   = NotifyWatcher::new(move || watch_binding.get(), what);

                core.when_changed.push(notifiable);

                Arc::new(watcher)
            }

            FloatingState::Value(binding) => {
                // Use a mapped binding to map the value and create a new watcher from that
                binding.map_binding(|value| FloatingState::Value(value)).watch(what)
            }

            FloatingState::Abandoned    => Arc::new(NotifyWatcher::new(move || FloatingState::Abandoned, what).0),
            FloatingState::Missing      => Arc::new(NotifyWatcher::new(move || FloatingState::Missing, what).0),
        }
    }
}

impl<TValue> FloatingBindingTarget<TValue> 
where
    TValue: 'static,
{
    ///
    /// Finishes the binding for this target
    ///
    pub fn finish_binding(self, binding: BindRef<TValue>) {
        // Take a weak reference to the core (this is used to call the notifications when the BindRef changes, which is the only way this binding can change once it has been bound)
        let weak_core = Arc::downgrade(&self.core);

        let to_notify = {
            // Fetch the core
            let mut core    = self.core.lock().unwrap();

            // Copy the values to be notified 
            let to_notify   = core.when_changed.iter().map(|notifiable| notifiable.clone_for_inspection()).collect::<Vec<_>>();
            if !to_notify.is_empty() {
                // Take ownership of the notifiables
                let mut to_notify       = core.when_changed.iter().map(|notifiable| notifiable.clone_for_inspection()).collect::<Vec<_>>();
                mem::swap(&mut to_notify, &mut core.when_changed);

                // Notify via the BindRef
                let bindref_changed     = binding.when_changed(notify(move || {
                    if let Some(core) = weak_core.upgrade() {
                        // Clean out any notifications that are no longer in use
                        to_notify.retain(|item| item.is_in_use());

                        // Notify anything that's left
                        to_notify.iter()
                            .for_each(|notifiable| { notifiable.mark_as_changed(); });

                        // Drop the notifications from the core if there's nothing left
                        if to_notify.is_empty() {
                            let mut core            = core.lock().unwrap();
                            core.binding_watcher    = None;
                        }
                    } else {
                        // Once the core is gone, there's nothing left to notify
                        to_notify = vec![];
                    }
                }));

                core.binding_watcher    = Some(bindref_changed);
            }

            // Update the binding
            core.binding    = FloatingState::Value(binding);

            to_notify
        };

        // Notify anything that's listening that the binding is available
        to_notify.into_iter()
            .for_each(|notifiable| { notifiable.mark_as_changed(); });

        // Everything else notifies via the new bindref from now on, so we don't need the when_changed list any more
        self.core.lock().unwrap().when_changed = vec![];
    }

    ///
    /// Indicates that the value to be bound is missing
    ///
    pub fn missing(self) {
        let to_notify = {
            let mut core = self.core.lock().unwrap();

            // Set the state to 'missing'
            core.binding = FloatingState::Missing;

            // Notify everything
            core.when_changed.iter().map(|notifiable| notifiable.clone_for_inspection()).collect::<Vec<_>>()
        };

        // Send out any needed notifications about the binding being abandoned
        to_notify.into_iter().for_each(|notifiable| { notifiable.mark_as_changed(); });
    }

}

impl<TValue> Drop for FloatingBindingTarget<TValue> {
    fn drop(&mut self) {
        let to_notify = {
            let mut core = self.core.lock().unwrap();

            // Set the state to 'abandoned' if it's currently 'waiting'
            if let FloatingState::Waiting = &core.binding {
                core.binding = FloatingState::Abandoned;

                // Notify everything
                core.when_changed.iter().map(|notifiable| notifiable.clone_for_inspection()).collect::<Vec<_>>()
            } else {
                // Nothing to notify
                vec![]
            }
        };

        // Send out any needed notifications about the binding being abandoned
        to_notify.into_iter().for_each(|notifiable| { notifiable.mark_as_changed(); });
    }
}

impl<TValue> FloatingBinding<TValue> 
where
    TValue: 'static + Clone + Send,
{
    ///
    /// Creates a new floating binding in the waiting state and a target for setting the final binding
    ///
    pub fn new() -> (FloatingBinding<TValue>, FloatingBindingTarget<TValue>) {
        // Create the core in a waiting state
        let core = FloatingBindingCore {
            binding:            FloatingState::Waiting,
            binding_watcher:    None,
            when_changed:       vec![],
        };
        let core    = Arc::new(Mutex::new(core));

        // Create the binding and its target
        let binding = FloatingBinding {
            core: Arc::clone(&core),
        };
        let target = FloatingBindingTarget {
            core: Arc::clone(&core),
        };

        (binding, target)
    }

    ///
    /// If this binding has been bound to a value, returns the underlying binding. Can also return 'None' if the
    /// binding is not yet available, or an error if the binding is unavaiable for any reason.
    ///
    /// This can be used in combination with `when_changed()` to wait for a binding to become available without
    /// using a future. Note that using `map_binding()` can also be used to supply a default value during the 
    /// period where the full binding is unavailable.
    ///
    pub fn try_get_binding(&self) -> Result<Option<BindRef<TValue>>, BindingError> {
        let core = self.core.lock().unwrap();

        match &core.binding {
            FloatingState::Abandoned    => { Err(BindingError::Abandoned) },
            FloatingState::Missing      => { Err(BindingError::Missing) },
            FloatingState::Value(value) => { Ok(Some(value.clone())) },
            FloatingState::Waiting      => { Ok(None) },
        }
    }

    ///
    /// Waits for the binding to become ready, returning the bound value once done
    ///
    /// It is also possible to use `when_changed()` or `watch()` in combination with `try_get_binding()` to wait for
    /// a binding without having to use a future. Another approach is to use `map_binding()` to map the various 'floating'
    /// values to default values while the binding is waiting to be fully bound.
    ///
    pub async fn wait_for_binding(self) -> Result<BindRef<TValue>, BindingError> {
        let (mut wait_for_binding, _monitor_lifetime) = {
            let core = self.core.lock().unwrap();

            // Short-circuit if the binding is already known or abandoned, otherwise follow updates to this binding
            match &core.binding {
                FloatingState::Abandoned    => { return Err(BindingError::Abandoned); },
                FloatingState::Missing      => { return Err(BindingError::Missing); },
                FloatingState::Value(value) => { return Ok(value.clone()); },
                FloatingState::Waiting      => {
                    // Add a notification every time the value in this object changes
                    let (mut send, recv)    = mpsc::channel(1);
                    let monitor_lifetime    = self.when_changed(notify(move || { send.try_send(()).ok(); }));

                    (recv, monitor_lifetime)
                },
            }
        };

        // Check the core every time the channel notifies us
        while let Some(()) = wait_for_binding.next().await {
            let core = self.core.lock().unwrap();

            // Keep waiting until the core leaves the 'waiting' state
            match &core.binding {
                FloatingState::Abandoned    => { return Err(BindingError::Abandoned); },
                FloatingState::Missing      => { return Err(BindingError::Missing); },
                FloatingState::Value(value) => { return Ok(value.clone()); },
                FloatingState::Waiting      => { }
            }
        }

        // If the stream ends, then the binding was presumably abandoned
        Err(BindingError::Abandoned)
    }
}

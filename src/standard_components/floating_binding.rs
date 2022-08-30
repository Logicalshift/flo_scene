use flo_binding::*;

use std::sync::*;

///
/// Core shared between a floating binding and its target
///
struct FloatingBindingCore<TValue> {
    /// The binding, once it has been supplied by the remote object
    binding: FloatingState<BindRef<TValue>>,
}

///
/// The possible state of a 'floating' binding value
///
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatingState<TValue> {
    /// The binding has not been set by the target yet
    Waiting,

    /// The target never bound the value
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

impl<TValue> Changeable for FloatingBinding<TValue> {
    fn when_changed(&self, what: Arc<dyn Notifiable>) -> Box<dyn Releasable> {
        unimplemented!()
    }
}

impl<TValue> Bound for FloatingBinding<TValue> {
    type Value = FloatingState<TValue>;

    #[inline]
    fn get(&self) -> Self::Value {
        unimplemented!()
    }

    #[inline]
    fn watch(&self, what: Arc<dyn Notifiable>) -> Arc<dyn Watcher<Self::Value>> {
        unimplemented!()
    }
}

#[cfg(feature="std")]
mod std_sharing_types {
    use std::sync::*;

    /// A shared reference type that can be cloned
    pub type Shared<T> = Arc<Mutex<T>>;

    /// A weak shared reference does not retain its contents if the main 'shared' items are released
    pub type WeakShared<T> = Weak<Mutex<T>>;

    /// Create a new shared item
    #[inline]
    pub fn share<T>(item: T) -> Shared<T> {
        Arc::new(Mutex::new(item))
    }

    /// Accesses a shared value
    #[inline]
    pub fn with_shared<T, TReturn>(shared: &Shared<T>, action: impl FnOnce(&mut T) -> TReturn) -> TReturn {
        let contents = shared.lock();

        if let Ok(mut contents) = contents {
            action(&mut *contents)
        } else {
            unreachable!()
        }
    }

    #[inline]
    pub fn shared_downgrade<T>(shared: &Shared<T>) -> WeakShared<T> {
        Arc::downgrade(shared)
    }

    /// Accesses a weak shared value, if possible
    #[inline]
    pub fn with_weak_shared<T, TReturn>(shared: &WeakShared<T>, action: impl FnOnce(&mut T) -> TReturn) -> Option<TReturn> {
        if let Some(shared) = shared.upgrade() {
            let contents = shared.lock();

            if let Ok(mut contents) = contents {
                Some(action(&mut *contents))
            } else {
                unreachable!()
            }
        } else {
            None
        }
    }
}

#[cfg(feature="one_thread")]
mod one_thread_sharing_types {
    use alloc::sync::*;
    use spin::{Mutex};

    /// A shared reference type that can be cloned
    pub type Shared<T> = Arc<Mutex<T>>;

    /// A weak shared reference does not retain its contents if the main 'shared' items are released
    pub type WeakShared<T> = Weak<Mutex<T>>;

    /// Create a new shared item
    #[inline]
    pub fn share<T>(item: T) -> Shared<T> {
        Arc::new(RefCell::new(item))
    }

    /// Accesses a shared value
    #[inline]
    pub fn with_shared<T, TReturn>(shared: &Shared<T>, action: impl FnOnce(&mut T) -> TReturn) -> TReturn {
        let contents = shared.lock();

        if let Ok(mut contents) = contents {
            action(&mut *contents)
        } else {
            unreachable!()
        }
    }

    /// Creates a weak version of a shared item
    #[inline]
    pub fn shared_downgrade<T>(shared: &Shared<T>) -> WeakShared<T> {
        Rc::downgrade(shared)
    }

    /// Accesses a weak shared value, if possible
    #[inline]
    pub fn with_weak_shared<T, TReturn>(shared: &WeakShared<T>, action: impl FnOnce(&mut T) -> TReturn) -> Option<TReturn> {
        if let Some(shared) = shared.upgrade() {
            let contents = shared.lock();

            if let Ok(mut contents) = contents {
                Some(action(&mut *contents))
            } else {
                unreachable!()
            }
        } else {
            None
        }
    }
}

#[cfg(feature="std")]
pub (crate) use std_sharing_types::*;

#[cfg(all(feature="one_thread", not(feature="std")))]
pub (crate) use one_thread_sharing_types::*;

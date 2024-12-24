#[cfg(feature="std")]
mod std_sharing_types {
    use std::sync::*;

    /// A shared reference type that can be cloned
    pub type Shared<T> = Arc<Mutex<T>>;

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
}

#[cfg(feature="one_thread")]
mod one_thread_sharing_types {
    use core::cell::*;
    use alloc::rc::*;

    /// A shared reference type that can be cloned
    pub type Shared<T> = Rc<RefCell<T>>;

    /// Accesses a shared value
    #[inline]
    pub fn with_shared<T, TReturn>(shared: &Shared<T>, action: impl FnOnce(&mut T) -> TReturn) -> TReturn {
        let contents = shared.try_borrow_mut();

        if let Ok(mut contents) = contents {
            action(&mut *contents)
        } else {
            unreachable!()
        }
    }
}

#[cfg(feature="std")]
pub (crate) use std_sharing_types::*;

#[cfg(all(feature="one_thread", not(feature="std")))]
pub (crate) use one_thread_sharing_types::*;

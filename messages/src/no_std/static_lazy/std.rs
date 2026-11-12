use core::ops::*;
use std::sync::*;

///
/// Type that is lazily initialised
///
/// (We need our own version of LazyLock to support no_std. This is the std version, which is just a wrapper around LazyLock)
///
pub struct StaticLazy<T, TNewFn = fn() -> T> {
    lazy_init: LazyLock<T, TNewFn>,
}

impl<T, TNewFn> StaticLazy<T, TNewFn>
where 
    TNewFn: 'static + Send + FnOnce() -> T,
{
    ///
    /// Create a new StaticLazy object
    ///
    pub const fn new(create_fn: TNewFn) -> Self {
        Self { 
            lazy_init: LazyLock::new(create_fn)            
        }
    }
}

impl<T, TNewFn> Deref for StaticLazy<T, TNewFn>
where 
    TNewFn: 'static + Send + FnOnce() -> T,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.lazy_init
    }
}

impl<T, TNewFn> DerefMut for StaticLazy<T, TNewFn>
where 
    TNewFn: 'static + Send + FnOnce() -> T,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.lazy_init
    }
}

///
/// Type that is lazily initialised
///
/// (We need our own version of LazyLock to support no_std. This is the no_std version, which uses a spinlock)
///
pub struct StaticOnce<T> {
    value:  OnceLock<T>,
}

impl<T> StaticOnce<T> {
    ///
    /// Create a new StaticLazy object
    ///
    pub const fn new() -> Self {
        Self { 
            value: OnceLock::new(),       
        }
    }

    ///
    /// Retrieves this value if it's been initialised
    ///
    #[inline]
    pub fn get(&self) -> Option<&T> {
        self.value.get()
    }

    ///
    /// Retrieves this value, initialising it if it's empty
    ///
    #[inline]
    pub fn get_or_init(&self, init: impl FnOnce() -> T) -> &T {
        self.value.get_or_init(init)
    }
}

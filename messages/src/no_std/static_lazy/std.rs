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

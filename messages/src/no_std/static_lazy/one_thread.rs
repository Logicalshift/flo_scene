use core::ops::*;
use spin;

///
/// Type that is lazily initialised
///
/// (We need our own version of LazyLock to support no_std. This is the no_std version, which uses a spinlock)
///
pub struct StaticLazy<T, TNewFn = fn() -> T> {
    init:   spin::Mutex<Option<TNewFn>>,
    value:  spin::Once<T>,
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
            init:   spin::Mutex::new(Some(create_fn)),
            value:  spin::Once::new(),       
        }
    }
}

impl<T, TNewFn> Deref for StaticLazy<T, TNewFn>
where 
    TNewFn: 'static + Send + FnOnce() -> T,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value.call_once(|| (self.init.lock().take().unwrap())())
    }
}

impl<T, TNewFn> DerefMut for StaticLazy<T, TNewFn>
where 
    TNewFn: 'static + Send + FnOnce() -> T,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value.call_once(|| (self.init.lock().take().unwrap())());
        self.value.get_mut().unwrap()
    }
}

///
/// Type that is lazily initialised
///
/// (We need our own version of LazyLock to support no_std. This is the no_std version, which uses a spinlock)
///
pub struct StaticOnce<T> {
    value:  spin::Once<T>,
}

impl<T> StaticOnce<T> {
    ///
    /// Create a new StaticLazy object
    ///
    pub const fn new() -> Self {
        Self { 
            value: spin::Once::new(),       
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
        self.value.call_once(init);
        self.get().unwrap()
    }
}

//!
//! Utility types for using flo_scene from a no_std environment.
//!

mod sharing_types;
mod ordered_vec;

pub use sharing_types::*;
pub use ordered_vec::*;

#[cfg(feature="one-thread")]
mod static_lazy {
    mod one_thread;
    pub use one_thread::*;
}

#[cfg(all(feature="use-std", not(feature="one-thread")))]
mod static_lazy {
    mod std;
    pub use std::*;
}

#[cfg(not(feature="use-std"))]
mod imports;
#[cfg(not(feature="use-std"))]
pub use imports::*;

#[cfg(not(feature="use-std"))]
pub use no_std::*;
pub use static_lazy::*;

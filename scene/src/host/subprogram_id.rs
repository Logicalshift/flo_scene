use once_cell::sync::{OnceCell};
use std::ops::{Deref};

pub use flo_scene_guest::host_types::{SubProgramId};

///
/// A static subprogram ID can be used to declare a subprogram ID in a static variable
///
pub struct StaticSubProgramId(&'static str, OnceCell<SubProgramId>);

impl StaticSubProgramId {
    ///
    /// Creates a subprogram ID with a well-known name
    ///
    #[inline]
    pub const fn called(name: &'static str) -> StaticSubProgramId {
        StaticSubProgramId(name, OnceCell::new())
    }
}

impl Deref for StaticSubProgramId {
    type Target = SubProgramId;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.1.get()
            .unwrap_or_else(|| {
                let subprogram = SubProgramId::called(self.0);
                self.1.set(subprogram).ok();
                self.1.get().unwrap()
            })
    }
}

#[cfg(not(target_family="wasm"))]
mod uuid_impl {
    use uuid::{Uuid};

    ///
    /// Generates a new UUID
    ///
    /// For WASM support we need to use an external function for this, but in other environments we just use `Uuid::new_v4`.
    /// Calling this function will choose the appropriate implementation for your environment. 
    ///
    /// Uuid::new_v4 is not usually available for flo_scene wasm modules as it uses wbindgen to get random valeus and these modules 
    /// are not usually run in a javascript environment.
    ///
    #[inline]
    pub fn new_uuid() -> Uuid {
        Uuid::new_v4()
    }
}

#[cfg(target_family="wasm")]
mod uuid_impl {
    use uuid::{Uuid};

    extern "C" {
        fn scene_request_new_uuid(bytes: &mut [u8; 16]);
    }

    ///
    /// Generates a new UUID
    ///
    /// For WASM support we need to use an external function for this, but in other environments we just use `Uuid::new_v4`.
    /// Calling this function will choose the appropriate implementation for your environment. 
    ///
    /// Uuid::new_v4 is not usually available for flo_scene wasm modules as it uses wbindgen to get random valeus and these modules 
    /// are not usually run in a javascript environment.
    ///
    #[inline]
    pub fn new_uuid() -> Uuid {
        let mut bytes = [0; 16];

        unsafe { scene_request_new_uuid(&mut bytes); }

        Uuid::from_bytes(bytes)
    }
}

pub use uuid_impl::*;

use serde::*;

///
/// The guest runtime handle is used by the host side to make requests to a runtime defined on the wasm side.
/// There can be more than one runtime if needed, though most scenarios can be executed using just a single
/// runtime. Runtimes can only use one message encoding strategy, so one reason that multiple might be used
/// is that multiple strategies are in use.
///
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
#[repr(transparent)]
pub struct GuestRuntimeHandle(pub usize);

///
/// Handle that identifies a subprogram running on the guest side
///
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct GuestSubProgramHandle(pub usize);

/// The default subprogram handle refers to the initial guest subprogram
impl Default for GuestSubProgramHandle {
    #[inline]
    fn default() -> Self {
        GuestSubProgramHandle(0)
    }
}

///
/// Handle to a buffer in a scene (these are used for transferring data to and from a webassembly module)
///
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
#[repr(transparent)]
pub struct BufferHandle(usize);

///
/// Handle that identifies an output sink on the host side
///
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct HostSinkHandle(pub usize);

///
/// The guest runtime handle is used by the host side to make requests to a runtime defined on the wasm side.
/// There can be more than one runtime if needed, though most scenarios can be executed using just a single
/// runtime. Runtimes can only use one message encoding strategy, so one reason that multiple might be used
/// is that multiple strategies are in use.
///
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
#[repr(transparent)]
pub struct GuestRuntimeHandle(pub usize);

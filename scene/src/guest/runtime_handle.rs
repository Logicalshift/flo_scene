///
/// The guest runtime handle is used by the host side to make requests to a runtime defined on the wasm side.
/// There can be more than one runtime if needed, though most scenarios can be executed using just a single
/// runtime. Runtimes can only use one message encoding strategy, so one reason that multiple might be used
/// is that multiple strategies are in use.
///
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
#[repr(transparent)]
pub struct GuestRuntimeHandle(pub usize);

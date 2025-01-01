///
/// Handle to a buffer in a scene (these are used for transferring data to and from a webassembly module)
///
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
#[repr(transparent)]
pub struct BufferHandle(pub (crate) usize);

///
/// A handle is used to identify a subprogram on the guest side
///
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestSubProgramHandle(pub usize);

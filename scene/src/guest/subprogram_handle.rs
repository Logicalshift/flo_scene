///
/// Handle that identifies a subprogram running on the guest side
///
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestSubProgramHandle(pub usize);

///
/// Handle that identifies a subprogram running on the guest side
///
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestSubProgramHandle(pub usize);

/// The default subprogram handle refers to the initial guest subprogram
impl Default for GuestSubProgramHandle {
    #[inline]
    fn default() -> Self {
        GuestSubProgramHandle(0)
    }
}
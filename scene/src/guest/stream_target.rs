use crate::subprogram_id::*;

///
/// Indicates where a stream should be connected on the host side from a guest
///
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostStreamTarget {
    None,
    Any,
    Program(SubProgramId)
}
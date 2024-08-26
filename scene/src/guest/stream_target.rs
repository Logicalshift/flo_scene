use super::stream_id::*;
use crate::subprogram_id::*;

///
/// Indicates where a stream should be connected on the host side from a guest
///
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostStreamTarget {
    None(HostStreamId),
    Any(HostStreamId),
    Program(SubProgramId, HostStreamId)
}

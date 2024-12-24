use super::host_stream_id::*;
use crate::host_types::*;

use serde::*;

///
/// Indicates where a stream should be connected on the host side from a guest
///
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum HostStreamTarget {
    None(HostStreamId),
    Any(HostStreamId),
    Program(SubProgramId, HostStreamId)
}

use crate::host::stream_id::*;

use flo_scene_guest::guest_types::*;

pub trait HostStreamTargetExt {
    ///
    /// Retrieves the stream ID, if there's a type within the current process that matches
    ///
    fn stream_id(&self) -> Option<StreamId>;
}

impl HostStreamTargetExt for HostStreamTarget {
    ///
    /// Retrieves the stream ID, if there's a type within the current process that matches
    ///
    #[inline]
    fn stream_id(&self) -> Option<StreamId> {
        StreamId::with_serialization_type(match self {
            HostStreamTarget::None(stream)          |
            HostStreamTarget::Any(stream)           |
            HostStreamTarget::Program(_, stream)    => &stream.0,
        })
    }
}

use crate::host::stream_id::*;
use crate::host::stream_target::*;

use flo_scene_guest::guest_types::*;

pub trait HostStreamTargetExt {
    ///
    /// Retrieves the stream ID, if there's a type within the current process that matches
    ///
    fn stream_id(&self) -> Option<StreamId>;

    ///
    /// Converts to a `StreamTarget` for use on the host instead of the guest
    ///
    fn to_stream_target(&self) -> StreamTarget;
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

    ///
    /// Converts to a `StreamTarget`
    ///
    #[inline]
    fn to_stream_target(&self) -> StreamTarget {
        match self {
            HostStreamTarget::None(_)                   => StreamTarget::None,
            HostStreamTarget::Any(_)                    => StreamTarget::Any,
            HostStreamTarget::Program(program_id, _)    => StreamTarget::Program(*program_id)
        }
    }
}

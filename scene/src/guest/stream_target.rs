use super::stream_id::*;
use crate::host::error::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

///
/// Indicates where a stream should be connected on the host side from a guest
///
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostStreamTarget {
    None(HostStreamId),
    Any(HostStreamId),
    Program(SubProgramId, HostStreamId)
}

impl HostStreamTarget {
    ///
    /// Changes a stream target into a host stream target if possible
    ///
    #[inline]
    pub fn from_stream_target<TMessageType>(target: impl Into<StreamTarget>) -> Result<HostStreamTarget, ConnectionError> 
    where
        TMessageType: SceneMessage,
    {
        let stream_id = HostStreamId::for_message::<TMessageType>();

        match target.into() {
            StreamTarget::None                  => Ok(HostStreamTarget::None(stream_id)),
            StreamTarget::Any                   => Ok(HostStreamTarget::Any(stream_id)),
            StreamTarget::Program(program_id)   => Ok(HostStreamTarget::Program(program_id, stream_id)),
            StreamTarget::Filtered(_, _)        => Err(ConnectionError::FilterNotSupported),
        }
    }

    ///
    /// Retrieves the stream ID, if there's a type within the current process that matches
    ///
    #[inline]
    pub fn stream_id(&self) -> Option<StreamId> {
        StreamId::with_serialization_type(match self {
            HostStreamTarget::None(stream)          |
            HostStreamTarget::Any(stream)           |
            HostStreamTarget::Program(_, stream)    => &stream.0,
        })
    }
}

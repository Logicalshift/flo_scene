use super::guest_message::*;
use super::stream_id::*;
use crate::host::error::*;
use crate::host::subprogram_id::*;
use crate::host::stream_target::*;

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
        TMessageType: GuestSceneMessage,
    {
        let stream_id = TMessageType::stream_id();

        match target.into() {
            StreamTarget::None                  => Ok(HostStreamTarget::None(stream_id)),
            StreamTarget::Any                   => Ok(HostStreamTarget::Any(stream_id)),
            StreamTarget::Program(program_id)   => Ok(HostStreamTarget::Program(program_id, stream_id)),
            StreamTarget::Filtered(_, _)        => Err(ConnectionError::FilterNotSupported),
        }
    }
}
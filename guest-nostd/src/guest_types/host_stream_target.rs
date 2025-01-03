use crate::errors::*;
use crate::host_types::*;
use super::host_stream_id::*;
use super::scene_guest_message::*;

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

pub trait ToHostStreamTarget {
    ///
    /// Converts this to a HostStreamTarget type (where the target stream has the given message type)
    ///
    fn to_host_stream_target<TMessageType: SceneGuestMessage>(self) -> Result<HostStreamTarget, ConnectionError>;
}

impl ToHostStreamTarget for HostStreamTarget {
    fn to_host_stream_target<TMessageType: SceneGuestMessage>(self) -> Result<HostStreamTarget, ConnectionError> {
        Ok(self)
    }
}

impl ToHostStreamTarget for () {
    fn to_host_stream_target<TMessageType: SceneGuestMessage>(self) -> Result<HostStreamTarget, ConnectionError> {
        let stream_id = HostStreamId::for_message::<TMessageType>();

        Ok(HostStreamTarget::Any(stream_id))
    }
}

impl ToHostStreamTarget for SubProgramId {
    fn to_host_stream_target<TMessageType: SceneGuestMessage>(self) -> Result<HostStreamTarget, ConnectionError> {
        let stream_id = HostStreamId::for_message::<TMessageType>();

        Ok(HostStreamTarget::Program(self, stream_id))
    }
}

impl HostStreamTarget {
    ///
    /// Changes a stream target into a host stream target if possible
    ///
    #[inline]
    pub fn from_stream_target<TMessageType>(target: impl Into<StreamTarget>) -> Result<HostStreamTarget, ConnectionError> 
    where
        TMessageType: SceneGuestMessage,
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
    /// Converts to a `StreamTarget`
    ///
    #[inline]
    pub fn to_stream_target(&self) -> StreamTarget {
        match self {
            HostStreamTarget::None(_)                   => StreamTarget::None,
            HostStreamTarget::Any(_)                    => StreamTarget::Any,
            HostStreamTarget::Program(program_id, _)    => StreamTarget::Program(*program_id)
        }
    }
}

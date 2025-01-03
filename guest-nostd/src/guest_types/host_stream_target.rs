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

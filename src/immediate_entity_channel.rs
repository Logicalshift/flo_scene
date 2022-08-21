use crate::error::*;
use crate::entity_channel::*;

///
/// Entity channel that can send messages and block on the current thread instead of requiring the
/// use of futures (a channel that works in 'immediate mode')
///
pub trait ImmediateEntityChannel : Send + EntityChannel {
    ///
    /// Sends a message to the channel and retrieves the response
    ///
    fn send_imm(&mut self, message: Self::Message) -> Result<Self::Response, EntityChannelError>;

    ///
    /// Sends a message to a channel where we don't want to wait for a response
    ///
    /// This is most useful for cases where the response is '()' - indeed, the version in `SceneContext` only supports
    /// this version. Not waiting for a response is often a faster way to dispatch messages, and also prevents deadlocks
    /// in the event that the message triggers a callback to the original entity. This also doesn't generate an error
    /// in the event the channel drops the message without responding to it.
    ///
    fn send_imm_without_waiting(&mut self, message: Self::Message) -> Result<(), EntityChannelError>;
}

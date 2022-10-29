use super::class::*;
use super::context::*;
use super::continuation::*;
use super::message::*;

///
/// A reference to the data for a class from the allocator
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TalkDataHandle(pub usize);

///
/// A reference to a data structure within a TalkContext
///
/// FloTalk data is stored by class and handle. References are only valid for the context that they were created for.
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TalkReference(pub (crate) TalkClass, pub (crate) TalkDataHandle);

impl TalkReference {
    ///
    /// Increases the reference count for this reference. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn add_reference(&self, context: &mut TalkContext) {
        context.get_callbacks(self.0).add_reference(self.1)
    }

    ///
    /// Decreases the reference count for this reference. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn remove_reference(&self, context: &mut TalkContext) {
        context.get_callbacks(self.0).remove_reference(self.1)
    }

    ///
    /// Sends a message to this object.
    ///
    #[inline]
    pub fn send_message_in_context(&self, message: TalkMessage, context: &mut TalkContext) -> TalkContinuation {
        context.get_callbacks(self.0).send_message(self.1, message)
    }
}

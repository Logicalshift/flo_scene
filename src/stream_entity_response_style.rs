///
/// Specifies how a stream entity processes messages
///
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StreamEntityResponseStyle {
    ///
    /// Send the message response as soon as each message is received
    ///
    /// This is the default style as it is the safest: when an entity sends messages before responding, there's a chance that those messages will themselves
    /// eventually end up waiting on the same entity, causing a deadlock. Responding immediately will break any message loops, at the cost of making it so
    /// the message sender cannot wait for the message to be fully processed before proceeding.
    ///
    RespondBeforeProcessing,

    ///
    /// Send the message response after processing the message
    ///
    /// This is most useful when the sender of a message needs to wait for it to be fully processed to continue.
    ///
    RespondAfterProcessing,
}

impl Default for StreamEntityResponseStyle {
    fn default() -> StreamEntityResponseStyle {
        StreamEntityResponseStyle::RespondBeforeProcessing
    }
}
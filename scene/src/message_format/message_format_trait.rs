use super::message_format::*;

///
/// Trait implemented by types that can describe their message format
///
pub trait HasMessageFormat {
    ///
    /// Returns the description of the format of this object when encoded into a message
    ///
    fn message_format() -> Option<MessageFormat>;
}
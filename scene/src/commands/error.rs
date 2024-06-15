///
/// An error that can be generated from a command
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandError {
    /// Error generated when an attempt is made to run a command that does not exist
    CommandNotFound(String),

    /// Managed to connect to the owner of the command but it produced an error when trying to send the message
    CommandFailedToRespond(String),

    /// The scene has no scene control program to query
    CannotQueryScene,
}

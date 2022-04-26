///
/// Errors that can occur while creating a default behaviour
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CreateDefaultError {
    /// Default behaviour is already defined for the requested message type
    AlreadyExists,
}

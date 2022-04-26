///
/// Errors that can occur while creating a new entity
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CreateEntityError {
    /// The entity that is being created already exists
    AlreadyExists,
}

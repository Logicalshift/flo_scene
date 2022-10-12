use super::entity_channel_error::*;

///
/// Errors that can occur while executing a recipe
///
#[derive(Clone, Debug, PartialEq)]
pub enum RecipeError {
    /// A channel that the recipe was trying to send to experienced an error
    ChannelError(EntityChannelError),

    /// A channel did not generate the response that was expected
    UnexpectedResponse,

    /// A channel expected more responses before it was dropped
    ExpectedMoreResponses,

    /// A recipe timed out before it could be completed
    Timeout,
}

impl From<EntityChannelError> for RecipeError {
    fn from(error: EntityChannelError) -> RecipeError {
        RecipeError::ChannelError(error)
    }
}

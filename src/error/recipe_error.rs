use super::entity_channel_error::*;

///
/// Errors that can occur while executing a recipe
///
#[derive(Clone, Debug, PartialEq)]
pub enum RecipeError {
    ChannelError(EntityChannelError)
}


impl From<EntityChannelError> for RecipeError {
    fn from(error: EntityChannelError) -> RecipeError {
        RecipeError::ChannelError(error)
    }
}

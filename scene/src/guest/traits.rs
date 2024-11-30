use super::poll_action::*;
use super::poll_result::*;

///
/// Trait implemented by a type that can communicate from the flo_scene host to the guest side
///
pub trait SceneGuest {
    ///
    /// Polls the guest with a request
    ///
    fn poll(action: GuestPollAction) -> GuestPollResult;
}

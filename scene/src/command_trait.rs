use crate::scene_context::*;
use crate::scene_message::*;

use futures::prelude::*;

///
/// Commands are spawnable tasks that carry out actions on behalf of a parent subprogram. A command can send multiple messages
/// to different targets and also can return a 'standard' output stream to to the subprogram that spawned it.
///
pub trait Command : Send {
    type Input:  'static + Send;
    type Output: 'static + SceneMessage;

    fn run(&self, input: impl 'static + Send + Stream<Item=Self::Input>, context: SceneContext) -> impl 'static + Send + Future<Output=()>;
}

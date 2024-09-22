use crate::scene_context::*;
use crate::scene_message::*;
use crate::commands::*;

use futures::prelude::*;

///
/// Commands are spawnable tasks that carry out actions on behalf of a parent subprogram. A command can send multiple messages
/// to different targets and also can return a 'standard' output stream to to the subprogram that spawned it.
///
pub trait Command : Send + Clone {
    type Input:  'static + SceneMessage;
    type Output: 'static + SceneMessage;

    fn run<'a>(&'a self, input: impl 'static + Send + Stream<Item=Self::Input>, context: SceneContext) -> impl 'a + Send + Future<Output=()>;
}

///
/// Extension functions that are implemented in terms of the standard command interface
///
pub trait CommandExt : Command {
    ///
    /// Creates a command that's the result of sending the output of this command to the input of another
    ///
    fn pipe_to<TTargetCommand: 'static + Command<Input=Self::Output>>(&self, target: TTargetCommand) -> PipeCommand<Self, TTargetCommand>;
}

impl<TCommand: 'static + Command> CommandExt for TCommand {
    ///
    /// Creates a new command that sends the input of this command to the output of another
    ///
    #[inline]
    fn pipe_to<TTargetCommand: 'static + Command<Input=Self::Output>>(&self, target: TTargetCommand) -> PipeCommand<Self, TTargetCommand> {
        PipeCommand::new(self.clone(), target)
    }
}

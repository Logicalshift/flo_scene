use crate::host::input_stream::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::commands::*;

use futures::prelude::*;

///
/// Commands are spawnable tasks that carry out actions on behalf of a parent subprogram. A command can send multiple messages
/// to different targets and also can return a 'standard' output stream to to the subprogram that spawned it.
///
pub trait Command : Send + Clone {
    /// The values that are passed in from the program that called it
    type Input:  'static + SceneMessage;

    /// The values that the command returns to the program that called it
    type Output: 'static + SceneMessage;

    /// Message type that can be sent to the command process ('()' is fine here if the command doesn't receive messages)
    type Message: 'static + SceneMessage;

    fn run<'a>(&'a self, command_input: impl 'static + Send + Stream<Item=Self::Input>, scene_messages: InputStream<Self::Message>, context: SceneContext) -> impl 'a + Send + Future<Output=()>;
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

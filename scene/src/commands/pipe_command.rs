use crate::command_trait::*;
use crate::scene_context::*;

use futures::prelude::*;

///
/// A pipe command takes the output of one command and sends it to the input of another
///
#[derive(Clone)]
pub struct PipeCommand<TSourceCommand, TTargetCommand>(TSourceCommand, TTargetCommand)
where
    TSourceCommand: Command,
    TTargetCommand: Command<Input=TSourceCommand::Output>;

impl<TSourceCommand, TTargetCommand> PipeCommand<TSourceCommand, TTargetCommand>
where
    TSourceCommand: 'static + Command,
    TTargetCommand: 'static + Command<Input=TSourceCommand::Output>,
{
    ///
    /// Creates a pipe that will send the output of `source` and send it to the input of `target` to generate the resulting output
    ///
    #[inline]
    pub fn new(source: TSourceCommand, target: TTargetCommand) -> Self {
        Self(source, target)
    }
}

impl<TSourceCommand, TTargetCommand> Command for PipeCommand<TSourceCommand, TTargetCommand>
where
    TSourceCommand: 'static + Command,
    TTargetCommand: 'static + Command<Input=TSourceCommand::Output>,
{
    type Input  = TSourceCommand::Input;
    type Output = TTargetCommand::Output;

    #[inline]
    fn run<'a>(&'a self, input: impl 'static + Send + Stream<Item=Self::Input>, context: SceneContext) -> impl 'a + Send + Future<Output=()> {
        let source_cmd = self.0.clone();
        let target_cmd = self.1.clone();

        async move {
            // Spawn the input as a separate command
            let pipe_stream = context.spawn_command(source_cmd, input);

            if let Ok(pipe_stream) = pipe_stream {
                // Pipe to the output (which inherits our context)
                target_cmd.run(pipe_stream, context).await;
            }
        }
    }
}

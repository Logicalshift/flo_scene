use crate::input_stream::*;
use crate::programs::QueryResponse;
use crate::scene_context::*;
use crate::scene_message::*;
use super::dispatcher::*;
use super::error::*;
use super::fn_command::*;
use super::list_commands::*;
use super::run_command::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::collections::{HashMap};
use std::marker::{PhantomData};
use std::sync::*;

///
/// A command launcher will response to `RunCommand<TParameter, Result<TResponse, CommandError>>` requests by
/// spawning a task using a function. This is the typical way that a group of command queries is declared.
///
/// The launcher will also respond to the `::list_commands` command with a list of responses converted from
/// the `ListCommandResponse` struture.
///
pub struct CommandLauncher<TParameter, TResponse> {
    /// The commands are invoked as a subtask when a `RunCommand<TParameter, Result<TResponse, CommandError>>` request is made
    commands: HashMap<String, Arc<dyn Send + Sync + Fn(&TParameter, SceneContext) -> BoxFuture<'static, ()>>>,

    response: PhantomData<TResponse>,
}

impl<TParameter, TResponse> CommandLauncher<TParameter, TResponse>
where
    TParameter: 'static + Unpin + Send + Sync,
    TResponse:  'static + Unpin + Send + Sync + SceneMessage + From<ListCommandResponse> + From<CommandError>,
{
    ///
    /// Creates a new command launcher, with no built in commands
    ///
    pub fn empty() -> Self {
        CommandLauncher {
            commands: HashMap::new(),
            response: PhantomData
        }
    }

    ///
    /// Returns this launcher modified with a new command. The command can send its results to the `TResponse` output stream in the context
    ///
    pub fn with_command<TFuture>(mut self, command_name: impl Into<String>, command: impl 'static + Send + Sync + Fn(&TParameter, SceneContext) -> TFuture) -> Self
    where
        TFuture: 'static + Send + Future<Output=()>,
    {
        self.commands.insert(command_name.into(), Arc::new(move |parameter, context| command(parameter, context).boxed()));

        self
    }

    ///
    /// Converts this launcher to a subprogram that can be added to a scene to respond to the run command requests
    ///
    pub fn to_subprogram(self) -> impl 'static + Send + FnOnce(InputStream<RunCommand<TParameter, TResponse>>, SceneContext) -> BoxFuture<'static, ()> {
        move |input, context| async move {
            let mut input = input;

            // Read run command requests from the input
            while let Some(run_request) = input.next().await {
                if run_request.name() == LIST_COMMANDS {
                    // List the commands in the launcher
                    let command_target = run_request.target();

                    let list_commands_response = QueryResponse::with_iterator(
                        self.commands.iter()
                        .map(|(name, _)| ListCommandResponse(name.clone()))
                        .chain([ListCommandResponse(LIST_COMMANDS.into())])
                        .map(|response| TResponse::from(response))
                        .collect::<Vec<_>>());

                    let response = context.send::<QueryResponse<TResponse>>(command_target);
                    
                    if let Ok(mut response) = response {
                        response.send(list_commands_response).await.ok();
                    }
                } else if let Some(command) = self.commands.get(run_request.name()).cloned() {
                    // Run the command
                    let command_target = run_request.target();
                    let command_output = context.spawn_command(FnCommand::<(), TResponse>::new(move |_, context| {
                        let command = Arc::clone(&command);
                        let future = (*command)(run_request.parameter(), context);

                        async move {
                            future.await
                        }
                    }), stream::empty());

                    if let Ok(command_output) = command_output {
                        // Send the output to the target
                        let response = context.send::<QueryResponse<TResponse>>(command_target);
                        
                        if let Ok(mut response) = response {
                            response.send(QueryResponse::with_stream(command_output)).await.ok();
                        }
                    }
                } else {
                    // Send an error saying the command is not known
                    let command_target  = run_request.target();
                    let command_name    = run_request.name().to_string();
                    let response        = context.send::<QueryResponse<TResponse>>(command_target);
                    
                    if let Ok(mut response) = response {
                        response.send(QueryResponse::with_iterator([TResponse::from(CommandError::CommandNotFound(command_name))])).await.ok();
                    }
                }
            }
        }.boxed()
    }
}

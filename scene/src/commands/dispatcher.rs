use super::run_command::*;
use super::list_commands::*;
use super::read_command::*;
use super::error::*;
use crate::input_stream::*;
use crate::scene_context::*;
use crate::subprogram_id::*;
use crate::programs::*;

use futures::prelude::*;

use std::collections::{HashMap, HashSet};

/// The name of the command sent to request the list command response
pub const LIST_COMMANDS: &str = "::list_commands";

///
/// Runs the command dispatcher subprogram for a particular type of command
///
/// This will find anything that is attached to the scene that is capable of running a command of the specified type by sending
/// the `LIST_COMMANDS` command to every subprogram that accepts a 'RunCommand' matching the type of this command dispatcher.
///
/// This request is only sent once to each subprogram to discover the commands they support: the way this works is that whenever
/// a command is sent to the dispatcher that it has not encountered before, it will scan the scene for any program that responds
/// to it. Once a command is associated with a program, this is not changed unless that program cannot be contacted when the command
/// is run again (in which case the commands are re-scanned)
///
/// This dispatcher itself also supports the list commands request, to list all of the commands found in all of the subprograms
/// in the scene.
///
pub async fn command_dispatcher_subprogram<TParameter, TResponse>(input: InputStream<RunCommand<TParameter, Result<TResponse, CommandError>>>, context: SceneContext)
where
    TParameter: 'static + Unpin + Send,
    TResponse:  'static + Unpin + Send + TryInto<ListCommandResponse>,
{
    let our_program_id = context.current_program_id().unwrap();

    // Create a hashmap of the known commands for the dispatcher
    let mut commands    = HashMap::<String, SubProgramId>::new();
    let mut subprograms = HashSet::<SubProgramId>::new();

    // Wait for requests to run commands
    let mut input = input;
    while let Some(next_command) = input.next().await {
        // Try to fetch the stream to send queries to the command owner
        let mut command_owner_stream = commands.get(next_command.name())
            .and_then(|command_owner| {
                context.send::<RunCommand<TParameter, Result<TResponse, CommandError>>>(*command_owner).ok()
            });

        // We might need to update our list of commands before evaluating this one
        // We update if the LIST_COMMANDS command is sent, or if the command is not known, or if there's no way to contact the existing target
        if next_command.name() == LIST_COMMANDS || command_owner_stream.is_none() {
            // Request the current scene status
            let scene_status = context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), ());
            let scene_status = if let Ok(scene_status) = scene_status {
                scene_status
            } else {
                // Can't list commands: need to reply with an error to the command target
                todo!();
                break;
            };

            let scene_status = scene_status.collect::<Vec<_>>().await;

            // TODO: remove any commands that belong to subprogram that are no longer in the list

            // TODO: Try to send a list command to each missing program in the scene and fill in the commands (except ourselves if we find ourselves)

            // Retry fetching the command owner stream
            command_owner_stream = commands.get(next_command.name())
                .and_then(|command_owner| {
                    context.send::<RunCommand<TParameter, Result<TResponse, CommandError>>>(*command_owner).ok()
                });
        }

        // Forward the command to the subprogram that listed it
        if let Some(command_owner_stream) = command_owner_stream {
            // Forward the command to the target stream
            let mut command_owner_stream = command_owner_stream;
            if let Ok(()) = command_owner_stream.send(next_command).await {
                // Command was sent OK, target should have responded
            } else {
                // Message was rejected for some reason: we should send an error
                //if let Ok(mut response_stream) = context.send::<QueryResponse<Result<TResponse, CommandError>>>(next_command.target()) {
                //    response_stream.send(QueryResponse::with_data(Err(CommandError::CommandFailedToRespond(next_command.name().into())))).await.ok();
                //}
                todo!()
            }
        } else {
            // This command is not known: send a query response indicating the error
            if let Ok(mut response_stream) = context.send::<QueryResponse<Result<TResponse, CommandError>>>(next_command.target()) {
                response_stream.send(QueryResponse::with_data(Err(CommandError::CommandNotFound(next_command.name().into())))).await.ok();
            }
        }
    }
}

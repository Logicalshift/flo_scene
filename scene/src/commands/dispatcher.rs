use super::run_command::*;
use super::list_commands::*;
use super::read_command::*;
use super::error::*;
use crate::input_stream::*;
use crate::scene_context::*;
use crate::scene_message::*;
use crate::subprogram_id::*;
use crate::programs::*;

use futures::prelude::*;

use std::collections::{HashMap, HashSet};
use std::iter;

/// The name of the command sent to request the list command response
pub const LIST_COMMANDS: &str = "list_commands";

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
pub async fn command_dispatcher_subprogram<TParameter, TResponse>(input: InputStream<RunCommand<TParameter, TResponse>>, context: SceneContext)
where
    TParameter: 'static + Unpin + Send + From<()>,
    TResponse:  'static + Unpin + Send + SceneMessage + TryInto<ListCommandResponse> + From<ListCommandResponse> + From<CommandError>,
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
                context.send::<RunCommand<TParameter, TResponse>>(*command_owner).ok()
            });

        // We might need to update our list of commands before evaluating this one
        // We update if the LIST_COMMANDS command is sent, or if the command is not known, or if there's no way to contact the existing target
        if next_command.name() == LIST_COMMANDS || command_owner_stream.is_none() {
            // Request the current scene status
            let scene_status = context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), ());
            let scene_status = if let Ok(scene_status) = scene_status {
                scene_status
            } else {
                // Can't list programs: need to reply with an error to the command target
                if let Ok(mut response) = context.send(next_command.target())
                {
                    response.send(TResponse::from(CommandError::CannotQueryScene)).await.ok();
                }

                // Fetch the next command
                continue;
            };

            let scene_status        = scene_status.collect::<Vec<_>>().await;

            // Figure out which subprograms have been removed or added
            let active_subprograms  = scene_status.iter().flat_map(|update| match update {
                SceneUpdate::Started(program_id, _) => Some(*program_id),
                _                                   => None,
            }).collect::<HashSet<SubProgramId>>();
            let removed_subprograms = subprograms.iter()
                .filter(|old_program| !active_subprograms.contains(old_program))
                .copied()
                .collect::<HashSet<_>>();
            let added_subprograms = active_subprograms.iter()
                .filter(|new_program| !subprograms.contains(new_program))
                .copied()
                .collect::<HashSet<_>>();

            // Remove any commands that belong to subprogram that are no longer in the list
            if !removed_subprograms.is_empty() {
                commands.retain(|_, program_id| !removed_subprograms.contains(program_id));
            }

            // Try to send a list command to each missing program in the scene and fill in the commands (except ourselves if we find ourselves)
            for added_program_id in added_subprograms.iter() {
                // Don't recursively request the list of programs
                if *added_program_id == our_program_id { continue; }

                // Send the LIST_COMMANDS command to the new program
                if let Ok(supported_commands) = context.spawn_query(ReadCommand::default(), RunCommand::<TParameter, TResponse>::new((), LIST_COMMANDS, ()), added_program_id) {
                    let mut supported_commands = supported_commands;
                    while let Some(cmd) = supported_commands.next().await {
                        // Convert to a list command response
                        let cmd: ListCommandResponse = if let Ok(cmd) = cmd.try_into() { cmd } else { continue; };

                        // Add this command to the known list if it's not present
                        if !commands.contains_key(&cmd.0) {
                            commands.insert(cmd.0, *added_program_id);
                        }

                        // TODO: also give the command a name that specifies the subprogram
                    }
                }
            }

            // Update the list of active subprograms
            subprograms = active_subprograms;

            // Retry fetching the command owner stream to determine the final command
            command_owner_stream = commands.get(next_command.name())
                .and_then(|command_owner| {
                    context.send::<RunCommand<TParameter, TResponse>>(*command_owner).ok()
                });
        }

        // Forward the command to the subprogram that listed it
        if next_command.name() == LIST_COMMANDS {
            // Respond with a list of commands (parameter is always ignored)
            if let Ok(mut response_stream) = context.send::<QueryResponse<TResponse>>(next_command.target()) {
                let command_names = commands.iter()
                    .map(|(name, _)| name.to_string())
                    .chain(iter::once(LIST_COMMANDS.into()))
                    .collect::<HashSet<_>>();

                response_stream.send(QueryResponse::with_iterator(command_names.into_iter().map(|name| ListCommandResponse(name).into()).collect::<Vec<_>>())).await.ok();
            }
        } else if let Some(command_owner_stream) = command_owner_stream {
            // Forward the command to the target stream
            let mut command_owner_stream = command_owner_stream;
            let command_target  = next_command.target();
            let command_name    = next_command.name().to_string();

            if let Ok(()) = command_owner_stream.send(next_command).await {
                // Command was sent OK, target should have responded
            } else {
                // Message was rejected for some reason: we should send an error
                if let Ok(mut response_stream) = context.send::<QueryResponse<TResponse>>(command_target) {
                    response_stream.send(QueryResponse::with_data(CommandError::CommandFailedToRespond(command_name).into())).await.ok();
                }
            }
        } else {
            // This command is not known: send a query response indicating the error
            if let Ok(mut response_stream) = context.send::<QueryResponse<TResponse>>(next_command.target()) {
                let command_name = next_command.name().to_string();
                response_stream.send(QueryResponse::with_data(CommandError::CommandNotFound(command_name).into())).await.ok();
            }
        }
    }
}

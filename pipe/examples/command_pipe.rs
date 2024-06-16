use flo_scene::*;
use flo_scene_pipe::*;
use flo_scene_pipe::commands::*;

use tokio;

use std::fs;

#[tokio::main]
async fn main() {
    // Delete the './example_unix_socket' file if it exists
    fs::remove_file("./example_unix_socket").ok();

    // Create a default scene
    let scene = Scene::default();

    // Create a unix socket that will run commands
    let command_program = SubProgramId::new();
    scene.add_subprogram(command_program, |input, context| command_connection_program(input, context, ()), 0);

    // The internal socket program lets us stream commands and responses via a socket connection
    let socket_program = SubProgramId::new();
    start_unix_socket_program(&scene, socket_program, "./example_unix_socket", parse_command_stream, display_command_responses).unwrap();

    // Connect the programs together
    scene.connect_programs(socket_program, command_program, StreamId::with_message_type::<CommandProgramSocketMessage>()).unwrap();

    // Run the scene
    println!("Created UNIX-domain socket at 'example_unix_socket'.\nTry 'tmux -S ./example_unix_socket' to connect.");
    println!();
    scene.run_scene().await;
}

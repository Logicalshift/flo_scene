use flo_scene::*;
use flo_scene::programs::*;
use flo_scene_pipe::*;

use futures::prelude::*;
use tokio::io::*;

use std::io::*;

#[test]
fn error_from_internal_socket() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
 
    struct TestSucceeded;
    impl SceneMessage for TestSucceeded { }

    // The command program accepts connections from the socket and interprets the commands
    let command_program = SubProgramId::new();
    scene.add_subprogram(command_program, command_connection_program, 0);

    // The internal socket program lets us stream commands and responses via a socket connection
    let socket_program = SubProgramId::new();
    start_internal_socket_program(&scene, socket_program, parse_command_stream, display_command_responses).unwrap();

    // Socket program is connected to the command program using the command program socket message (which generates connections)
    scene.connect_programs(socket_program, command_program, StreamId::with_message_type::<CommandProgramSocketMessage>()).unwrap();

    // Add another program that talks to the command program via a socket connection
    scene.add_subprogram(SubProgramId::new(), |_input: InputStream<()>, context| async move {
        // Crete a message to send
        let test_commands = "error::message\n";
        let test_commands = test_commands.bytes().collect::<Vec<_>>();
        let test_commands = Cursor::new(test_commands);

        // Also create an internal buffer to write to
        let (read_result, write_result) = duplex(1024);

        // Request that the socket program read from the test commands and writes to the internal buffer
        let mut socket_program = context.send(socket_program).unwrap();
        socket_program.send(InternalSocketMessage::CreateInternalSocket(Box::new(test_commands), Box::new(write_result))).await.ok().unwrap();

        // Read the interal buffer to get the final result
        let mut read_result = read_result;
        while let Ok(msg) = read_result.read_u8().await {
            println!("{:?}", msg as char);
        }

        println!("DONE");

        // Indicate successs
        context.send_message(TestSucceeded).await.ok();
    }, 0);

    // Wait for the test program to indicate that it succeeded
    TestBuilder::new()
        .redirect_input(StreamId::with_message_type::<TestSucceeded>())
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene_with_threads(&scene, test_program, 5);
}
use flo_scene::*;
use flo_scene::programs::*;
use flo_scene_pipe::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::channel::oneshot;

use serde_json;

#[test]
pub fn send_error_command() {
    let scene = Scene::default();

    struct TestSucceeded;
    impl SceneMessage for TestSucceeded { }

    // Create a basic command program
    let test_program    = SubProgramId::new();
    let command_program = SubProgramId::new();
    scene.add_subprogram(command_program, command_connection_program, 0);

    // Test that we can send some messages to it
    scene.add_subprogram(SubProgramId::called("Test"), |_: InputStream<()>, context| async move {
        let (send_commands, recv_commands)      = mpsc::channel(1);
        let (send_responses, recv_responses)    = oneshot::channel();

        // Request a connection
        println!("Request connection...");
        let connection = SocketConnection::new(&context, recv_commands, move |_context, output| { send_responses.send(output).ok(); });
        context.send(command_program).unwrap().send(CommandProgramSocketMessage::Connection(connection)).await.ok().unwrap();

        // Get the response stream
        println!("Wait for connection...");
        let mut send_commands   = send_commands;
        let mut response_stream = recv_responses.await.unwrap();

        // Send an error command
        println!("Send command...");
        let command = CommandRequest::parse("example::doesnotexist").await;
        send_commands.send(command).await.unwrap();

        // Retrieve the response
        println!("Receive...");
        let error_response = response_stream.next().await.unwrap();
        println!("  ...{:?}", error_response);
        assert!(matches!(&error_response, CommandResponse::Error(_)), "{:?}", error_response);

        // Send the 'succeded' message
        context.send_message(TestSucceeded).await.unwrap();
    }, 0);

    // Run a test that just waits for the 'succeeded' message
    TestBuilder::new()
        .redirect_input(StreamId::with_message_type::<TestSucceeded>())
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene_with_threads(&scene, test_program, 5);
}

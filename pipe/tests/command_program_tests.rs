use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;
use flo_scene_pipe::*;
use flo_scene_pipe::commands::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::channel::oneshot;

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

#[test]
pub fn error_for_nonexistent_json_command() {
    let scene           = Scene::default();
    let test_subprogram = SubProgramId::new();

    // We can send a JSON command as a query, and it should make it to the default dispatcher. If we use a known invalid command it should return an error.
    TestBuilder::new()
        .run_query(ReadCommand::default(), JsonCommand::new((), "::not-a-command", serde_json::Value::Null), (), |output| {
            // Should be an error response
            if output.len() != 1 { return Err(format!("Output is {:?}", output)); }
            if !matches!(&output[0], CommandResponse::Error(_)) { return Err(format!("Output is {:?}", output)); }

            Ok(())
        })
        .run_in_scene(&scene, test_subprogram);
}

#[test]
pub fn declare_and_run_json_command() {
    let scene           = Scene::default();
    let test_subprogram = SubProgramId::new();
    let command_program = SubProgramId::new();

    // Create a command launcher program that just parrots strings back to us
    let json_launcher = CommandLauncher::json()
        .with_json_command("::test", |param: String, _context| async move {
            CommandResponse::Json(vec![serde_json::Value::String(param)])
        });
    scene.add_subprogram(command_program, json_launcher.to_subprogram(), 1);

    // Try running this command (the dispatcher should start and find the subprogram for us).
    TestBuilder::new()
        .run_query(ReadCommand::default(), JsonCommand::new((), "::test", serde_json::Value::String("Hello".to_string())), (), |output| {
            // Should just send the string back to us
            if output.len() != 1 { return Err(format!("Output is {:?}", output)); }

            match &output[0] {
                CommandResponse::Json(values) => {
                    if values != &vec![serde_json::Value::String("Hello".to_string())] {
                        return Err(format!("Output is {:?}", output));
                    }
                }

                _ => {
                    return Err(format!("Output is {:?}", output));
                }
            }

            Ok(())
        })
        .run_in_scene(&scene, test_subprogram);
}

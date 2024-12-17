use flo_scene::*;
use flo_scene::commands::*;
use flo_scene::programs::*;
use flo_scene_pipe::*;
use flo_scene_pipe::commands::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::channel::oneshot;

use serde::*;
use serde_json::json;
use tokio::io::*;

///
/// Creates an internal socket program in a scene that can be used to send commands
///
fn create_internal_command_socket(scene: &Scene, internal_socket_id: SubProgramId) {
    // The command connection program receives connections from sockets
    let command_program = SubProgramId::new();
    scene.add_subprogram(command_program, |input, context| command_connection_program(input, context, ()), 0);

    // The internal socket program lets us receive connections and send messages to the command program as streams of data
    start_internal_socket_program(scene, internal_socket_id, read_command_data, write_command_data).unwrap();

    // Connect the internal socket program to the command program
    scene.connect_programs(internal_socket_id, command_program, StreamId::with_message_type::<CommandProgramSocketMessage>()).unwrap();
}

///
/// Adds a subprogram that runs some commands using the internal socket program
///
fn add_command_runner<TFuture>(scene: &Scene, internal_socket_id: SubProgramId, commands: impl Into<String>, process_results: impl 'static + Send + Fn(String, SceneContext) -> TFuture) 
where
    TFuture: 'static + Send + Future<Output=()>
{
    // Create an arbitrary program ID
    let program_id  = SubProgramId::called("command_runner");
    let commands    = commands.into();

    scene.add_subprogram(program_id, move |_: InputStream<()>, context| async move {
        context.wait_for_idle(100).await;

        // Create a connection via the internal socket
        let (our_side, their_side)          = duplex(1024);
        let (command_input, command_output) = split(their_side);
        let (read_result, write_command)    = split(our_side);

        let mut socket_program = context.send(internal_socket_id).unwrap();
        socket_program.send(InternalSocketMessage::CreateInternalSocket(Box::new(command_input), Box::new(command_output))).await.ok().unwrap();

        let context = &context;

        // Future that writes the commands
        let write_side = async move {
            println!("In: {}", commands);

            // Send the commands to the write side and then stop
            let mut write_command = write_command;

            write_command.write_all(&commands.bytes().collect::<Vec<u8>>()).await.unwrap();

            println!("Sent all");

            context.wait_for_idle(100).await;

            write_command.flush().await.unwrap();
            write_command.shutdown().await.unwrap();

            println!("Finished sending");
        };

        // Future that reads the results and processes them
        let read_side = async move {
            let mut bytes = vec![];

            let mut read_result = read_result;
            let mut buf = vec![];
            while let Ok(len) = read_result.read_buf(&mut buf).await {
                println!("{:?}", String::from_utf8_lossy(&buf));
                bytes.extend(&buf);
                buf.drain(..);

                if len == 0 {
                    break;
                }
            }

            let string_result = String::from_utf8_lossy(&bytes);
            println!("\nOut: {}", string_result);
            process_results(string_result.into(), context.clone()).await;
        };

        // Wait for both futures together to run the socket
        future::join(write_side, read_side).await;
    }, 0)
}

#[test]
pub fn send_error_command() {
    let scene = Scene::default();

    #[derive(Serialize, Deserialize)]
    struct TestSucceeded;
    impl SceneMessage for TestSucceeded { }

    // Create a basic command program
    let test_program    = SubProgramId::new();
    let command_program = SubProgramId::new();
    scene.add_subprogram(command_program, |input, context| command_connection_program(input, context, ()), 0);

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
        send_commands.send("example::doesnotexist\n".into()).await.unwrap();

        // Retrieve the response
        println!("Receive until error...");
        while let Some(error_response) = response_stream.next().await {
            let error_response = String::from_utf8_lossy(&error_response.0);
            println!("  ...{:?}", error_response);
            if error_response.contains("\n!!! ") {
                break;
            }
        }

        // Send the 'succeded' message
        context.send_message(TestSucceeded).await.unwrap();
    }, 0);

    // Run a test that just waits for the 'succeeded' message
    TestBuilder::new()
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
pub fn error_for_nonexistent_json_command() {
    let scene           = Scene::default();
    let test_subprogram = SubProgramId::new();

    // We can send a JSON command as a query, and it should make it to the default dispatcher. If we use a known invalid command it should return an error.
    TestBuilder::new()
        .run_query(ReadCommand::default(), JsonCommand::new((), "::not-a-command", serde_json::Value::Null, None), (), |output| {
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
            CommandResponse::Json(serde_json::Value::String(param))
        });
    scene.add_subprogram(command_program, json_launcher.to_subprogram(), 1);

    // Try running this command (the dispatcher should start and find the subprogram for us).
    TestBuilder::new()
        .run_query(ReadCommand::default(), JsonCommand::new((), "::test", serde_json::Value::String("Hello".to_string()), None), (), |output| {
            // Should just send the string back to us
            if output.len() != 1 { return Err(format!("Output is {:?}", output)); }

            match &output[0] {
                CommandResponse::Json(values) => {
                    if values != &serde_json::Value::String("Hello".to_string()) {
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

#[test]
pub fn pipe_command_io_stream() {
    let scene               = Scene::default().with_standard_json_commands();
    let test_subprogram     = SubProgramId::called("test");
    let command_subprogram  = SubProgramId::called("command");
    let internal_socket     = SubProgramId::called("send_internal_socket");

    // Create a launcher with a command we can pipe from and one we can pipe to
    let launcher = CommandLauncher::json()
        .with_json_command("::add_one", |_param: (), _context| async move {
            CommandResponse::IoStream(Box::new(|input_values| {
                use serde_json::{Value};

                input_values.map(|value| {
                    match value {
                        Value::Number(num) => json![num.as_f64().unwrap() + 1.0],
                        _ => json!["Unexpected value"]
                    }
                }).boxed()
            }))
        });
    scene.add_subprogram(command_subprogram, launcher.to_subprogram(), 1);

    // Pipe between the two commands using the interpreter
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"::add_one | ::add_one
1.0
2.0
3.0
42.0
.
        "#, 
        |msg, _| async move {
            // We're hard-coding the JSON formatting here which might not always be consistent (many formats can communicate the same message)
            println!("Msg is {:?}", msg);
            assert!(msg.contains(r#"<< JSON <<

3.0

4.0

5.0

44.0
== JSON ==
"#));
        });

    // Pipe from one command to another and check the results
    TestBuilder::new()
        .run_in_scene(&scene, test_subprogram);
}

#[test]
pub fn pipe_command_json_output() {
    let scene               = Scene::default().with_standard_json_commands();
    let test_subprogram     = SubProgramId::called("test");
    let command_subprogram  = SubProgramId::called("command");
    let internal_socket     = SubProgramId::called("send_internal_socket");

    // Create a launcher with a command we can pipe from and one we can pipe to
    let launcher = CommandLauncher::json()
        .with_json_command("::pipe_from", |_param: (), _context| async move {
            CommandResponse::Json(json![42])
        })
        .with_json_command("::pipe_to", |_param: (), _context| async move {
            CommandResponse::IoStream(Box::new(|input_values| {
                use serde_json::{Value};

                input_values.map(|value| {
                    match value {
                        Value::Number(num) => json![num.as_f64().unwrap() + 1.0],
                        _ => json!["Unexpected value"]
                    }
                }).boxed()
            }))
        });
    scene.add_subprogram(command_subprogram, launcher.to_subprogram(), 1);

    // Pipe between the two commands using the interpreter
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"::pipe_from | ::pipe_to
        "#, 
        |msg, _| async move {
            // We're hard-coding the JSON formatting here which might not always be consistent (many formats can communicate the same message)
            println!("Msg is {:?}", msg);
            assert!(msg.contains(r#"<< JSON <<

43.0
== JSON ==
"#));
        });

    // Pipe from one command to another and check the results
    TestBuilder::new()
        .run_in_scene(&scene, test_subprogram);
}

#[test]
pub fn pipe_command_background_stream() {
    let scene               = Scene::default().with_standard_json_commands();
    let test_subprogram     = SubProgramId::called("test");
    let command_subprogram  = SubProgramId::called("command");
    let internal_socket     = SubProgramId::called("send_internal_socket");

    // Create a launcher with a command we can pipe from and one we can pipe to
    let launcher = CommandLauncher::json()
        .with_json_command("::pipe_from", |_param: (), _context| async move {
            CommandResponse::BackgroundStream(stream::iter(vec![json![1], json![2], json![3], json![42]]).boxed())
        })
        .with_json_command("::pipe_to", |_param: (), _context| async move {
            CommandResponse::IoStream(Box::new(|input_values| {
                use serde_json::{Value};

                input_values.map(|value| {
                    match value {
                        Value::Number(num) => json![num.as_f64().unwrap() + 1.0],
                        _ => json!["Unexpected value"]
                    }
                }).boxed()
            }))
        });
    scene.add_subprogram(command_subprogram, launcher.to_subprogram(), 1);

    // Pipe between the two commands using the interpreter
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"::pipe_from | ::pipe_to
        "#, 
        |msg, _| async move {
            // We're hard-coding the JSON formatting here which might not always be consistent (many formats can communicate the same message)
            println!("Msg is {:?}", msg);
            assert!(msg.contains(r#"<< JSON <<

2.0

3.0

4.0

43.0
== JSON ==
"#));
        });

    // Pipe from one command to another and check the results
    TestBuilder::new()
        .run_in_scene(&scene, test_subprogram);
}

#[test]
pub fn pipe_command_chain() {
    let scene               = Scene::default().with_standard_json_commands();
    let test_subprogram     = SubProgramId::called("test");
    let command_subprogram  = SubProgramId::called("command");
    let internal_socket     = SubProgramId::called("send_internal_socket");

    // Create a launcher with a command we can pipe from and one we can pipe to
    let launcher = CommandLauncher::json()
        .with_json_command("::add_one", |_param: (), _context| async move {
            CommandResponse::IoStream(Box::new(|input_values| {
                use serde_json::{Value};

                input_values.map(|value| {
                    match value {
                        Value::Number(num) => json![num.as_f64().unwrap() + 1.0],
                        _ => json!["Unexpected value"]
                    }
                }).boxed()
            }))
        }).with_json_command("::double", |_param: (), _context| async move {
            CommandResponse::IoStream(Box::new(|input_values| {
                use serde_json::{Value};

                input_values.map(|value| {
                    match value {
                        Value::Number(num) => json![num.as_f64().unwrap() * 2.0],
                        _ => json!["Unexpected value"]
                    }
                }).boxed()
            }))
        });
    scene.add_subprogram(command_subprogram, launcher.to_subprogram(), 1);

    // Pipe between the two commands using the interpreter
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"::add_one | ::add_one | ::double
1.0
2.0
3.0
42.0
.
        "#, 
        |msg, _| async move {
            // We're hard-coding the JSON formatting here which might not always be consistent (many formats can communicate the same message)
            println!("Msg is {:?}", msg);
            assert!(msg.contains(r#"<< JSON <<

6.0

8.0

10.0

88.0
== JSON ==
"#));
        });

    // Pipe from one command to another and check the results
    TestBuilder::new()
        .run_in_scene(&scene, test_subprogram);
}

#[test]
fn substitute_parameter() {
    let scene               = Scene::default().with_standard_json_commands();
    let test_subprogram     = SubProgramId::called("test");
    let command_subprogram  = SubProgramId::called("command");
    let internal_socket     = SubProgramId::called("send_internal_socket");

    // Create a launcher with some simple mathematical functions that take their values as integers
    let launcher = CommandLauncher::json()
        .with_json_command("::add_one", |param: i64, _context| async move {
            CommandResponse::Json(json!{param + 1})
        }).with_json_command("::double", |param: i64, _context| async move {
            CommandResponse::Json(json!{param * 2})
        });
    scene.add_subprogram(command_subprogram, launcher.to_subprogram(), 1);

    // Pipe between the two commands using the interpreter
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"::add_one <::double <::add_one 4>>
        "#, 
        |msg, _| async move {
            // We're hard-coding the JSON formatting here which might not always be consistent (many formats can communicate the same message)
            println!("Msg is {:?}", msg);
            assert!(msg.contains(r#" 11
"#));
        });

    // Pipe from one command to another and check the results
    TestBuilder::new()
        .run_in_scene(&scene, test_subprogram);
}

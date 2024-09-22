use flo_scene::*;
use flo_scene::programs::*;
use flo_scene_pipe::*;
use flo_scene_pipe::commands::*;

use futures::prelude::*;
use serde::*;
use tokio::io::*;

/// TestSucceeded message is used to indicate when a test has passed
#[derive(Serialize, Deserialize, Debug)]
struct TestSucceeded { message: String }
impl SceneMessage for TestSucceeded {
    fn message_type_name() -> String { "test::TestSucceeded".into() }
}

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
fn send_command() {
    let scene           = Scene::default().with_standard_json_commands();
    let internal_socket = SubProgramId::called("send_internal_socket");
    let test_program    = SubProgramId::called("send_test_program");
 
    // Set up the internal socket and the test case
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"send { "Type": "test::TestSucceeded" }
        { "message": "test 1" }
        { "message": "test 2" }
        "#, 
        |_, _| async { });

    // Create a test program that receives the TestSucceeded message
    TestBuilder::new()
        .expect_message(|_: TestSucceeded| Ok(()))
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene(&scene, test_program);
}

#[test]
fn echo_command() {
    let scene           = Scene::default().with_standard_json_commands();
    let internal_socket = SubProgramId::called("echo_internal_socket");
    let test_program    = SubProgramId::called("echo_test_program");
 
    // Set up the internal socket and the test case
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"echo "Hello"
        "#, 
        move |msg, context| async move {
            assert!(msg.contains("   Hello\n"), "{}", msg);
            context.send(test_program).unwrap().send(TestSucceeded { message: "Ok".into() }).await.unwrap();
        });

    // Create a test program that receives the TestSucceeded message
    TestBuilder::new()
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene(&scene, test_program);
}

#[test]
fn echo_variable() {
    let scene           = Scene::default().with_standard_json_commands();
    let internal_socket = SubProgramId::called("echo_internal_socket");
    let test_program    = SubProgramId::called("echo_test_program");

    // Test case is to assign a variable value and then echo it
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#":test = "Hello"
        echo :test
        "#, 
        move |msg, context| async move {
            assert!(msg.contains("   Hello\n"), "{}", msg);
            context.send(test_program).unwrap().send(TestSucceeded { message: "Ok".into() }).await.unwrap();
        });

    // Create a test program that receives the TestSucceeded message
    TestBuilder::new()
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene(&scene, test_program);
}

#[test]
fn echo_array_variable() {
    let scene           = Scene::default().with_standard_json_commands();
    let internal_socket = SubProgramId::called("echo_internal_socket");
    let test_program    = SubProgramId::called("echo_test_program");

    // Test case is to assign a variable value and then substitute it in an array and echo it
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#":test = "Hello"
        echo [ :test, :test ]
        "#, 
        move |msg, context| async move {
            assert!(msg.contains("   Hello\n   Hello\n"), "{}", msg);
            context.send(test_program).unwrap().send(TestSucceeded { message: "Ok".into() }).await.unwrap();
        });

    // Create a test program that receives the TestSucceeded message
    TestBuilder::new()
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene(&scene, test_program);
}

#[test]
fn echo_object_variable() {
    let scene           = Scene::default().with_standard_json_commands();
    let internal_socket = SubProgramId::called("echo_internal_socket");
    let test_program    = SubProgramId::called("echo_test_program");

    // Test case is to assign a variable value and then substitute it in an object and echo it (object formatting syntax might change, which this test doesn't account for at the moment)
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#":test = "Hello"
        echo { "test": :test }
        "#, 
        move |msg, context| async move {
            assert!(msg.contains("   {\n     \"test\": \"Hello\"\n   }"), "{}", msg);
            context.send(test_program).unwrap().send(TestSucceeded { message: "Ok".into() }).await.unwrap();
        });

    // Create a test program that receives the TestSucceeded message
    TestBuilder::new()
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene(&scene, test_program);
}

#[test]
fn subscribe_command() {
    let scene               = Scene::default().with_standard_json_commands();
    let internal_socket     = SubProgramId::called("send_internal_socket");
    let subscribe_program   = SubProgramId::called("subscribe_program");
    let test_program        = SubProgramId::called("send_test_program");

    // The message we send for the subscription
    #[derive(Serialize, Deserialize, Debug)]
    struct SubscribeCommandTestMessage {
        text: String,
    }

    impl SceneMessage for SubscribeCommandTestMessage {
        fn message_type_name() -> String { "test::SubscribeCommandTestMessage".into() }
    }

    // Create a program that we can subscribe to
    scene.add_subprogram(subscribe_program, |input, context| async move {
        let mut input           = input;
        let mut subscription    = EventSubscribers::<SubscribeCommandTestMessage>::new();

        while let Some(req) = input.next().await {
            let req: Subscribe<SubscribeCommandTestMessage> = req;

            subscription.subscribe(&context, req.target());
            subscription.send_round_robin(SubscribeCommandTestMessage { text: "Test".into() }).await.ok();
        }
    }, 0);

    scene.connect_programs((), subscribe_program, StreamId::with_message_type::<Subscribe<SubscribeCommandTestMessage>>()).unwrap();

    // Test case is to ask to subscribe to the program we just created, and check for the results arriving back again
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"subscribe { "Type": "test::SubscribeCommandTestMessage" }
        "#, 
        |msg, _| async move {
            // We're hard-coding the JSON formatting here which might not always be consistent (many formats can communicate the same message)
            assert!(msg.contains("\n<<< 0\n"));
            assert!(msg.contains("\n<0 {\n  \"text\": \"Test\"\n}\n"));
        });

    // Create a test program that receives the TestSucceeded message
    TestBuilder::new()
        .run_in_scene(&scene, test_program);
}

#[test]
fn query_command() {
    let scene               = Scene::default().with_standard_json_commands();
    let internal_socket     = SubProgramId::called("send_internal_socket");
    let query_program       = SubProgramId::called("query_program");
    let test_program        = SubProgramId::called("send_test_program");

    // The message we send for the query
    #[derive(Serialize, Deserialize, Debug)]
    struct QueryCommandTestMessage {
        text: String,
    }

    impl SceneMessage for QueryCommandTestMessage {
        fn message_type_name() -> String { "test::QueryCommandTestMessage".into() }
    }

    // Create a program that we can query
    scene.add_subprogram(query_program, |input, context| async move {
        let mut input           = input;

        while let Some(req) = input.next().await {
            let req: Query<QueryCommandTestMessage> = req;

            let response    = QueryResponse::with_iterator(vec![QueryCommandTestMessage { text: "a".into() }, QueryCommandTestMessage { text: "b".into() }]);
            let mut sender  = context.send(req.target()).unwrap();
            sender.send(response).await.ok();
        }
    }, 0);

    scene.connect_programs((), query_program, StreamId::with_message_type::<Query<QueryCommandTestMessage>>()).unwrap();

    // Test case is to query the program we created
    create_internal_command_socket(&scene, internal_socket);
    add_command_runner(&scene, internal_socket, 
        r#"query { "Type": "test::QueryCommandTestMessage" }
        "#, 
        |msg, _| async move {
            // We're hard-coding the JSON formatting here which might not always be consistent (many formats can communicate the same message)
            assert!(msg.contains(r#"[
  {
    "text": "a"
  },
  {
    "text": "b"
  }
]"#));
        });

    // Create a test program that receives the TestSucceeded message
    TestBuilder::new()
        .run_in_scene(&scene, test_program);
}

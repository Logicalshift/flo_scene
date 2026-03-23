use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::commands::*;

use futures::prelude::*;
use serde::*;

#[test]
fn simple_command() {
    let scene = Scene::default();

    // Create a test command that sends some usize values to its output
    let test_command = FnCommand::<(), usize>::new(|_input, context| async move {
        // Connect the usize output
        let mut output = context.send::<usize>(()).unwrap();

        // Send some output data
        output.send(1).await.unwrap();
        output.send(2).await.unwrap();
        output.send(3).await.unwrap();
        output.send(4).await.unwrap();
    });

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    TestBuilder::new()
        .run_command(test_command.clone(), vec![], |output| if &output != &vec![1, 2, 3, 4] { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn simple_query() {
    let scene = Scene::default();

    // The result for our test query
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestQuery(usize);

    impl SceneMessage for TestQuery { }

    // Request type for the query
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestQueryRequest(StreamTarget);

    impl SceneMessage for TestQueryRequest { }

    impl QueryRequest for TestQueryRequest {
        type ResponseData = TestQuery;

        fn with_new_target(self, new_target: StreamTarget) -> Self {
            Self(new_target)
        }
    }

    // Create a subprogram to respond to queries
    let test_subprogram = SubProgramId::called("QueryTest");

    scene.add_subprogram(test_subprogram, |input, context| async move {
        let mut input = input;

        while let Some(TestQueryRequest(target)) = input.next().await {
            let mut response = context.send(target).unwrap();

            response.send(QueryResponse::with_iterator(vec![
                TestQuery(1),
                TestQuery(2),
                TestQuery(3),
                TestQuery(4),
            ])).await.ok();
        }
    }, 100);

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    TestBuilder::new()
        .run_query(ReadCommand::default(), TestQueryRequest(StreamTarget::None), test_subprogram, |output| if &output != &vec![TestQuery(1), TestQuery(2), TestQuery(3), TestQuery(4)] { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn simple_query_x1000() {
    let scene = Scene::default();

    // The result for our test query
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestQuery(usize);

    impl SceneMessage for TestQuery { }

    // Request type for the query
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestQueryRequest(StreamTarget);

    impl SceneMessage for TestQueryRequest { }

    impl QueryRequest for TestQueryRequest {
        type ResponseData = TestQuery;

        fn with_new_target(self, new_target: StreamTarget) -> Self {
            Self(new_target)
        }
    }

    // Create a subprogram to respond to queries
    let test_subprogram = SubProgramId::called("QueryTest");

    scene.add_subprogram(test_subprogram, |input, context| async move {
        let mut input = input;

        while let Some(TestQueryRequest(target)) = input.next().await {
            let mut response = context.send(target).unwrap();

            response.send(QueryResponse::with_iterator(vec![
                TestQuery(1),
                TestQuery(2),
                TestQuery(3),
                TestQuery(4),
            ])).await.ok();
        }
    }, 100);

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    let mut test = TestBuilder::new();

    for _ in 0..1000 {
        test = test.run_query(ReadCommand::default(), TestQueryRequest(StreamTarget::None), test_subprogram, |output| if &output != &vec![TestQuery(1), TestQuery(2), TestQuery(3), TestQuery(4)] { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
    }

    test.run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn simple_command_x1000() {
    let scene = Scene::default();

    // Create a test command that sends some usize values to its output
    let test_command = FnCommand::<(), usize>::new(|_input, context| async move {
        // Connect the usize output
        let mut output = context.send::<usize>(()).unwrap();

        // Send some output data
        output.send(1).await.unwrap();
        output.send(2).await.unwrap();
        output.send(3).await.unwrap();
        output.send(4).await.unwrap();
    });

    // Run the command using the test builder
    let test_program = SubProgramId::new();

    let mut test = TestBuilder::new();

    for _ in 0..1000 {
        test = test.run_command(test_command.clone(), vec![], |output| if &output != &vec![1, 2, 3, 4] { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) });
    }

    test.run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn pipe_command() {
    let scene = Scene::empty();

    // Create a test command that sends some usize values to its output
    let test_command = FnCommand::<(), usize>::new(|_input, context| async move {
        // Connect the usize output
        let mut output = context.send::<usize>(()).unwrap();

        // Send some output data
        println!("send(1)");
        output.send(1).await.unwrap();
        println!("send(2)");
        output.send(2).await.unwrap();
        println!("send(3)");
        output.send(3).await.unwrap();
        println!("send(4)");
        output.send(4).await.unwrap();
        println!("done.");
    });

    let add_one_command = FnCommand::<usize, usize>::new(|input, context| async move {
        let mut input  = input;
        let mut output = context.send::<usize>(()).unwrap();

        // Add one to the input
        println!("+1 start");
        while let Some(next) = input.next().await {
            println!("+1: {:?}", next);
            output.send(next+1).await.unwrap();
            println!("  = {:?}", next+1);
        }
        println!("+1 done");
    });

    let combined_command = test_command.pipe_to(add_one_command);

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    TestBuilder::new()
        .run_command(combined_command.clone(), vec![], |output| if &output != &vec![2, 3, 4, 5] { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn query_command() {
    let scene = Scene::default();

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|_: IdleNotification| { Ok(()) })
        .run_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), *SCENE_CONTROL_PROGRAM, |output| if output.len() == 0 { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn connect_filter_source_in_command() {
    let scene = Scene::default();

    #[derive(Serialize, Deserialize)]
    struct TestMessage { }

    impl SceneMessage for TestMessage { }

    let filter_handle = FilterHandle::for_filter::<String, _>(|src| src.map(|_| TestMessage { }));

    // Create a test command that sends some usize values to its output
    let cmd_scene = scene.clone();
    let test_command = FnCommand::<(), usize>::new(move |_input, context| {
        let cmd_scene       = cmd_scene.clone();
        let filter_handle   = filter_handle.clone();
        async move {
            // Connect the usize output
            let mut output = context.send::<usize>(()).unwrap();

            // Send some output data
            println!("Sending initial values");
            output.send(1).await.unwrap();
            output.send(2).await.unwrap();

            // Create a new connection
            // The command output connects to an input stream that doesn't belong to a program, so if it gets reconnected as a side-effect, the messages can end up going nowhere
            println!("Reconnecting");
            cmd_scene.connect_programs(StreamSource::Filtered(filter_handle), StreamTarget::Any, StreamId::with_message_type::<String>()).unwrap();

            // Finish sending
            println!("Sending remaining values");            
            output.send(3).await.unwrap();
            output.send(4).await.unwrap();
            println!("Done");
        }
    });

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    TestBuilder::new()
        .run_command(test_command.clone(), vec![], |output| if &output != &vec![1, 2, 3, 4] { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

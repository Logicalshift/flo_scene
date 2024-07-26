//!
//! The control program can be used to start new programs and stop them from within
//! a program in a scene. It's started when a scene is created with `Scene::default()`.
//! It's an optional program, so a scene that does not need to be dynamic or which has
//! its own method of controlling when it starts and stops can start with `Scene::empty()`
//!

use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::commands::*;

use futures::prelude::*;
use futures::future::{select};
use futures::executor;
use futures_timer::*;

use std::collections::*;
use std::time::{Duration};
use std::sync::*;

#[test]
fn ask_control_to_stop_scene() {
    // The default scene has the 'control' program in it
    let scene       = Scene::default();
    scene.add_subprogram(
        SubProgramId::new(),
        move |input: InputStream<()>, context| async move {
            // Tell it to stop the stream
            context.send_message(SceneControl::StopScene).await.unwrap();

            // Read from our input forever
            let mut input = input;
            while let Some(_) = input.next().await {

            }
        },
        0,
    );

    let mut has_stopped = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        has_stopped = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have stopped the scene and not just timed out
    assert!(has_stopped, "Scene did not stop");
}

#[test]
fn ask_control_to_stop_scene_when_idle() {
    // The default scene has the 'control' program in it
    let scene       = Scene::default();
    scene.add_subprogram(
        SubProgramId::new(),
        move |input: InputStream<()>, context| async move {
            // Tell it to stop the stream when it's idle
            context.send_message(SceneControl::StopSceneWhenIdle).await.unwrap();

            // Read from our input forever
            let mut input = input;
            while let Some(_) = input.next().await {

            }
        },
        0,
    );

    let mut has_stopped = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        has_stopped = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have stopped the scene and not just timed out
    assert!(has_stopped, "Scene did not stop");
}

#[test]
fn ask_control_to_start_program() {
    let has_started = Arc::new(Mutex::new(false));

    // The default scene has the 'control' program in it
    let scene           = Scene::default();
    let notify_started  = has_started.clone();
    scene.add_subprogram(
        SubProgramId::new(),
        move |input: InputStream<()>, context| async move {
            // Tell it to start a new program
            context.send_message(SceneControl::start_program(SubProgramId::new(), move |_: InputStream<()>, context| {
                let notify_started = notify_started.clone();
                async move {
                    // Set the flag to indicate the new program started
                    *notify_started.lock().unwrap() = true;

                    // Stop the scene as the test is done
                    context.send_message(SceneControl::StopScene).await.unwrap();
                }
            }, 0)).await.unwrap();

            // Read from our input forever
            let mut input = input;
            while let Some(_) = input.next().await {

            }
        },
        0,
    );

    let mut has_stopped = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        has_stopped = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have started the subprogram, then stopped the scene
    assert!(*has_started.lock().unwrap() == true, "Subprogram did not start");
    assert!(has_stopped, "Scene did not stop");
}

#[test]
fn ask_control_to_connect_and_close_programs() {
    // The default scene has the 'control' program in it
    let scene           = Scene::default();

    // We have three programs: one that receives messages, one that sends messages and one that connects them together
    let recv_messages = Arc::new(Mutex::new(vec![]));

    let sender_program      = SubProgramId::new();
    let receiver_program    = SubProgramId::new();
    let connection_program  = SubProgramId::new();

    // The receiver program adds its input to the recv_messages list
    let sent_messages       = recv_messages.clone();
    scene.add_subprogram(
        receiver_program, 
        move |mut input: InputStream<String>, _| async move {
            // Read input for as long as the stream is open
            while let Some(input) = input.next().await {
                println!("Received");
                sent_messages.lock().unwrap().push(input);
            }

            // Stop the scene once the input is stopped
            println!("Closed, stopping");
            scene_context().unwrap().send_message(SceneControl::StopScene).await.unwrap(); 
        }, 
        0);

    // The sender program sends some strings, then closes the output stream of the receiver program
    scene.add_subprogram(
        sender_program, 
        move |_: InputStream<()>, context| async move {
            let mut string_output = context.send::<String>(()).unwrap();

            println!("Send 1...");
            string_output.send("1".to_string()).await.unwrap();
            println!("Send 2...");
            string_output.send("2".to_string()).await.unwrap();
            println!("Send 3...");
            string_output.send("3".to_string()).await.unwrap();
            println!("Send 4...");
            string_output.send("4".to_string()).await.unwrap();

            println!("Close stream");
            context.send_message(SceneControl::Close(receiver_program)).await.unwrap();
        }, 
        0);

    // The connection prorgam creates a new connection from the sender to the receiver
    scene.add_subprogram(
        connection_program, 
        move |_: InputStream<()>, context| async move {
            println!("Requesting connect...");
            context.send_message(SceneControl::connect(sender_program, receiver_program, StreamId::with_message_type::<String>())).await.unwrap();
            println!("Requested");
        },
        0);

    let mut has_stopped = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        has_stopped = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have sent the messages, closed down the receiver, and then stopped the scene
    let recv_messages = (*recv_messages.lock().unwrap()).clone();

    assert!(recv_messages == vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string()], "Did not send the correct messages to the receiver (receiver got {:?})", recv_messages);
    assert!(has_stopped, "Scene did not stop");
}

#[test]
fn scene_update_messages() {
    // Create a scene with two subprograms. Program_1 will send to Program_2
    let scene       = Scene::default();
    let program_1   = SubProgramId::new();
    let program_2   = SubProgramId::new();

    // Create a program to monitor the updates for the scene
    let update_monitor  = SubProgramId::new();
    let recv_updates    = Arc::new(Mutex::new(vec![]));
    let send_updates    = recv_updates.clone();
    scene.add_subprogram(update_monitor,
        move |mut input: InputStream<SceneUpdate>, _| async move {
            let mut program_1_finished = false;
            let mut program_2_finished = false;

            while let Some(input) = input.next().await {
                println!("--> {:?}", input);

                match &input {
                    SceneUpdate::Stopped(stopped_program) => {
                        if *stopped_program == program_1 { program_1_finished = true; }
                        if *stopped_program == program_2 { program_2_finished = true; }
                    }

                    _ => {}
                }

                send_updates.lock().unwrap().push(input);

                if program_1_finished && program_2_finished {
                    break;
                }
            }

            // Stop the scene once the two test programs are finished
            scene_context().unwrap().send_message(SceneControl::StopScene).await.unwrap();
        },
        0);
    scene.connect_programs((), update_monitor, StreamId::with_message_type::<SceneUpdate>()).unwrap();

    // program_1 reads from its input and sets it in sent_message
    scene.add_subprogram(program_1,
        move |mut input: InputStream<usize>, _| async move {
            // Read a single message and write it to the 'sent_message' structure
            input.next().await.unwrap();
        },
        0);

    // program_2 sends a message to program_1 directly (by requesting a stream for program_1)
    scene.add_subprogram(program_2,
        move |_: InputStream<()>, context| async move {
            let mut send_usize = context.send::<usize>(program_1).unwrap();
            send_usize.send(42).await.unwrap();
        },
        0);

    // Run this scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Check that the updates we expected are generated for this program
    let recv_updates = recv_updates.lock().unwrap().drain(..).collect::<Vec<_>>();
    assert!(has_finished, "Scene did not terminate properly");

    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Started(prog_id, _) => *prog_id == program_1, _ => false }).count() == 1,
        "Program 1 started more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Started(prog_id, _) => *prog_id == program_2, _ => false }).count() == 1,
        "Program 2 started more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Stopped(prog_id) => *prog_id == program_1, _ => false }).count() == 1,
        "Program 1 stopped more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Stopped(prog_id) => *prog_id == program_2, _ => false }).count() == 1,
        "Program 2 stopped more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Connected(src, tgt, _strm) => *src == program_2 && *tgt == program_1, _ => false }).count() == 1,
        "Program 2 connected the wrong number of times to program 1");
}

#[test]
fn scene_update_messages_using_subscription() {
    // Create a scene with two subprograms. Program_1 will send to Program_2
    let scene       = Scene::default();
    let program_1   = SubProgramId::new();
    let program_2   = SubProgramId::new();

    // Create a program to monitor the updates for the scene
    let update_monitor  = SubProgramId::new();
    let recv_updates    = Arc::new(Mutex::new(vec![]));
    let send_updates    = recv_updates.clone();
    scene.add_subprogram(update_monitor,
        move |mut input: InputStream<SceneUpdate>, context| async move {
            let mut program_1_finished = false;
            let mut program_2_finished = false;

            context.send_message(subscribe::<SceneUpdate>(context.current_program_id().unwrap())).await.unwrap();

            while let Some(input) = input.next().await {
                println!("--> {:?}", input);

                match &input {
                    SceneUpdate::Stopped(stopped_program) => {
                        if *stopped_program == program_1 { program_1_finished = true; }
                        if *stopped_program == program_2 { program_2_finished = true; }
                    }

                    _ => {}
                }

                send_updates.lock().unwrap().push(input);

                if program_1_finished && program_2_finished {
                    break;
                }
            }

            // Stop the scene once the two test programs are finished
            scene_context().unwrap().send_message(SceneControl::StopScene).await.unwrap();
        },
        0);

    // TODO: something better than these delays for figuring out when to stop the two extra programs (we can't get the input type for programs that are already stopped when the subscription starts)

    // program_1 reads from its input and sets it in sent_message
    scene.add_subprogram(program_1,
        move |mut input: InputStream<usize>, _| async move {
            // Read a single message and write it to the 'sent_message' structure
            input.next().await.unwrap();
            Delay::new(Duration::from_millis(100)).await;
        },
        0);

    // program_2 sends a message to program_1 directly (by requesting a stream for program_1)
    scene.add_subprogram(program_2,
        move |_: InputStream<()>, context| async move {
            let mut send_usize = context.send::<usize>(program_1).unwrap();
            send_usize.send(42).await.unwrap();
            Delay::new(Duration::from_millis(100)).await;
        },
        0);

    // Run this scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Check that the updates we expected are generated for this program
    let recv_updates = recv_updates.lock().unwrap().drain(..).collect::<Vec<_>>();
    assert!(has_finished, "Scene did not terminate properly");

    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Started(prog_id, _) => *prog_id == program_1, _ => false }).count() == 1,
        "Program 1 started more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Started(prog_id, _) => *prog_id == program_2, _ => false }).count() == 1,
        "Program 2 started more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Stopped(prog_id) => *prog_id == program_1, _ => false }).count() == 1,
        "Program 1 stopped more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Stopped(prog_id) => *prog_id == program_2, _ => false }).count() == 1,
        "Program 2 stopped more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Connected(src, tgt, _strm) => *src == program_2 && *tgt == program_1, _ => false }).count() == 1,
        "Program 2 connected the wrong number of times to program 1");
}

#[test]
fn query_control_program() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();

    scene.add_subprogram(program_1,
        move |mut input: InputStream<()>, _| async move {
            input.next().await.unwrap();
        },
        0);

    // Need to make sure that the query happens after the control program has had time to load the initial set of programs: using a timeout for this at the moment
    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|IdleNotification| { Ok(()) })
        .run_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), *SCENE_CONTROL_PROGRAM, 
            move |response| {
                if response.is_empty() { return Err("No updates in query response".to_string()); }
                if !response.iter().any(|update| update == &SceneUpdate::Started(program_1, StreamId::with_message_type::<()>())) { return Err(format!("Program 1 ({:?}) not in query response ({:?})", program_1, response)); }
                if !response.iter().any(|update| update == &SceneUpdate::Started(*SCENE_CONTROL_PROGRAM, StreamId::with_message_type::<SceneControl>())) { return Err(format!("Scene control program not in query response ({:?})", response)); }

                Ok(()) 
            })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn send_message_only_sends_one_connection_notification() {
    let scene           = Scene::default();
    let test_program_id = SubProgramId::new();
    let monitor_program = SubProgramId::new();
    let send_messages   = SubProgramId::new();
    let recv_messages   = SubProgramId::new();

    // The send program has finished sending, and waited for the scene to become idle
    #[derive(Debug)]
    struct SendFinish;
    impl SceneMessage for SendFinish { }

    #[allow(dead_code)]
    #[derive(Debug)]
    struct TestMessage(usize);
    impl SceneMessage for TestMessage { }

    #[allow(dead_code)]
    #[derive(Debug)]
    struct ReceivedUpdate(SceneUpdate);
    impl SceneMessage for ReceivedUpdate { }

    // The monitor program subscribes to scene updates and then forwards any connection messages for the recv_messages program to the test program
    scene.add_subprogram(monitor_program, move |input: InputStream<SceneUpdate>, context| async move {
        let mut test_program = context.send(test_program_id).unwrap();

        // Request scene control messages
        context.send_message(SceneControl::Subscribe(monitor_program.into())).await.unwrap();

        let mut input = input;
        while let Some(next_message) = input.next().await {
            // Forward any connection messages for the TestMessage type to the test program
            match &next_message {
                SceneUpdate::Connected(source, _, target_stream) => {
                    if *source == send_messages && target_stream.as_message_type() == StreamId::with_message_type::<TestMessage>() {
                        println!("{:?}", next_message);

                        test_program.send(ReceivedUpdate(next_message.clone())).await.unwrap();
                    }
                }

                _ => {}
            }
        }
    }, 0);

    // The recv subprogram receives messages and is the default target for the test message
    scene.add_subprogram(recv_messages, move |input: InputStream<TestMessage>, _context| async move {
        let mut input = input;
        while let Some(_msg) = input.next().await {
            // Nothing to do wtih the messages
        }
    }, 100);
    scene.connect_programs((), recv_messages, StreamId::with_message_type::<TestMessage>()).unwrap();

    // The send subprogram sends a couple of messages then tells the test program when it's done
    scene.add_subprogram(send_messages, move |_: InputStream<()>, context| async move {
        context.wait_for_idle(100).await;

        // First should generate a connection request, second should not
        context.send_message(TestMessage(0)).await.unwrap();
        context.send_message(TestMessage(1)).await.unwrap();

        context.wait_for_idle(100).await;

        context.send_message(TestMessage(3)).await.unwrap();
        context.send_message(TestMessage(4)).await.unwrap();

        // Wait for the messages to all finish processing
        context.wait_for_idle(100).await;

        // Tell the test program we're done
        context.send(test_program_id).unwrap().send(SendFinish).await.unwrap();
    }, 0);

    // Should receive a single connection and a finish message
    TestBuilder::new()
        .expect_message(|_: ReceivedUpdate| Ok(()))
        .expect_message(|_: SendFinish| Ok(()))
        .run_in_scene_with_threads(&scene, test_program_id, 5);
}

#[test]
fn sending_scene_update_to_stopped_program_does_not_block() {
    for _ in 0..20 {
        // When a program is stopped and is subscribed to the scene updates, the scene control program should not block waiting to tell it that
        // it has closed down
        let scene               = Scene::default();

        let test_program_id     = SubProgramId::new();
        let subscriber_program  = SubProgramId::called("test::subscriber_program");
        let query_program       = SubProgramId::called("test::query_program");

        // The subscriber program sends all of the 'subscribe' messages sent before the scene becomes idle (after subscribing)
        #[derive(Debug)]
        #[allow(dead_code)]
        enum SubscriberProgramMessage {
            SceneUpdate(SceneUpdate),
            IdleNotification(IdleNotification),
        }

        impl SceneMessage for SubscriberProgramMessage { }

        let update_filter   = FilterHandle::for_filter(|stream| stream.map(|msg| SubscriberProgramMessage::SceneUpdate(msg)));
        let idle_filter     = FilterHandle::for_filter(|stream| stream.map(|msg| SubscriberProgramMessage::IdleNotification(msg)));

        scene.add_subprogram(subscriber_program, move |input, context| async move {
            // Before we do anything we wait for the scene to become idle
            context.wait_for_idle(100).await;

            // Subscribe to the scene update events, then wait for the scene to become idle
            context.send_message(SceneControl::Subscribe(context.current_program_id().unwrap().into())).await.unwrap();
            context.send_message(IdleRequest::WhenIdle(context.current_program_id().unwrap().into())).await.unwrap();

            // Read the updates until the scene becomes idle
            let mut input   = input;
            while let Some(update) = input.next().await {
                match update {
                    SubscriberProgramMessage::SceneUpdate(_)        => { },
                    SubscriberProgramMessage::IdleNotification(_)   => { break; }
                }
            }

            // Send the updates to the query program
            context.send(query_program).unwrap()
                .send(QueryProgramMessage::Ready)
                .await
                .unwrap();

            // Finish the subscriber program (control program should not get stuck sending requests to it)
            println!("Finishing subscriber program");
        }, 0);

        scene.connect_programs((), StreamTarget::Filtered(update_filter, subscriber_program), StreamId::with_message_type::<SceneUpdate>()).unwrap();
        scene.connect_programs((), StreamTarget::Filtered(idle_filter, subscriber_program), StreamId::with_message_type::<IdleNotification>()).unwrap();

        // The query program receives the information from the subscription program, and then runs a query to see if the results match (with some known differences)
        #[derive(Debug)]
        enum QueryProgramMessage {
            Ready
        }

        impl SceneMessage for QueryProgramMessage { }

        scene.add_subprogram(query_program, move |input, context| async move {
            // Wait for the query program to do its work
            let mut input = input;
            while let Some(msg) = input.next().await {
                match msg {
                    QueryProgramMessage::Ready => { break; }
                }
            }

            // Query the current state of the scene (this creates more 'connected' messages which can block the scene control program)
            println!("Querying status...");
            let query = context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), ()).unwrap();
            let query = query.collect::<HashSet<_>>().await;
            println!("Query done: {} updates", query.len());

            context.send(test_program_id).unwrap()
                .send(TestResult::Ready).await.unwrap();
        }, 1);

        // Test checks that there were only expected differences
        #[derive(Debug)]
        enum TestResult {
            Ready
        }

        impl SceneMessage for TestResult { }

        TestBuilder::new()
            .expect_message(|_result: TestResult| { 
                // The scene control program should not get blocked, so should respond to the query
                Ok(())
            })
            .run_in_scene_with_threads(&scene, test_program_id, 5);
    }
}

#[test]
fn subscription_events_match_query_messages() {
    // When you subscribe to the control program it will list the active running programs and connections
    // When you query it, it does the same thing (except it will sometimes return programs and connections that haven't been sent to subscribers yet)
    // These two sets should match (with the exception of the subtasks created by the query program)
    let scene               = Scene::default();

    let test_program_id     = SubProgramId::called("test::test_program");
    let subscriber_program  = SubProgramId::called("test::subscriber_program");
    let query_program       = SubProgramId::called("test::query_program");

    // The subscriber program sends all of the 'subscribe' messages sent before the scene becomes idle (after subscribing)
    #[derive(Debug)]
    enum SubscriberProgramMessage {
        SceneUpdate(SceneUpdate),
        IdleNotification(IdleNotification),
    }

    impl SceneMessage for SubscriberProgramMessage { }

    let update_filter   = FilterHandle::for_filter(|stream| stream.map(|msg| SubscriberProgramMessage::SceneUpdate(msg)));
    let idle_filter     = FilterHandle::for_filter(|stream| stream.map(|msg| SubscriberProgramMessage::IdleNotification(msg)));

    scene.add_subprogram(subscriber_program, move |input, context| async move {
        let mut input   = input;

        // Before we do anything we wait for the scene to become idle
        context.wait_for_idle(100).await;

        // Warm up by requesting an idle notification and waiting for it (otherwise there's a potential race where the idle program can connect to us after the 'subscribe' events have been sent, so we miss the extra event that is generated)
        context.send_message(IdleRequest::WhenIdle(context.current_program_id().unwrap().into())).await.unwrap();
        while let Some(update) = input.next().await {
            match update {
                SubscriberProgramMessage::IdleNotification(_) => { break; }
                _ => { }
            }
        }

        // Subscribe to the scene update events, then wait for the scene to become idle
        context.send_message(SceneControl::Subscribe(context.current_program_id().unwrap().into())).await.unwrap();
        context.send_message(IdleRequest::WhenIdle(context.current_program_id().unwrap().into())).await.unwrap();

        // We'll use this connection to send the results onwards later on (we do want it to be present in the results)
        let mut send_to_query = context.send(query_program).unwrap();

        // Read the updates until the scene becomes idle
        let mut updates = vec![];
        while let Some(update) = input.next().await {
            match update {
                SubscriberProgramMessage::SceneUpdate(update)   => { updates.push(update); },
                SubscriberProgramMessage::IdleNotification(_)   => { break; }
            }
        }

        // Send the updates to the query program
        println!("Updates done: {} updates", updates.len());
        send_to_query
            .send(QueryProgramMessage::Updates(updates))
            .await
            .unwrap();

        // Keep running while the query test is run (so the connections from this program will appear in the results)
        while let Some(extra_input) = input.next().await {
            println!("Extra input: {:?}", extra_input);
        }

        println!();
        println!("Finishing subscriber program");
    }, 0);

    scene.connect_programs((), StreamTarget::Filtered(update_filter, subscriber_program), StreamId::with_message_type::<SceneUpdate>()).unwrap();
    scene.connect_programs((), StreamTarget::Filtered(idle_filter, subscriber_program), StreamId::with_message_type::<IdleNotification>()).unwrap();

    // The query program receives the information from the subscription program, and then runs a query to see if the results match (with some known differences)
    #[derive(Debug)]
    enum QueryProgramMessage {
        Updates(Vec<SceneUpdate>),
    }

    impl SceneMessage for QueryProgramMessage { }

    scene.add_subprogram(query_program, move |input, context| async move {
        // Wait for the query program to do its work
        let mut updates = HashSet::new();

        let mut input = input;
        while let Some(msg) = input.next().await {
            match msg {
                QueryProgramMessage::Updates(received_updates) => { 
                    updates = received_updates.into_iter().collect::<HashSet<_>>(); 
                    println!("\nReceived updates");
                    break;
                }
            }
        }

        // Query the current state of the scene
        println!("Querying status...");
        let query = context.spawn_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), ()).unwrap();
        let query = query.collect::<HashSet<_>>().await;
        println!("Query done: {} updates", query.len());

        let added_updates   = query.iter().filter(|msg| !updates.contains(*msg)).cloned().collect::<Vec<_>>();
        let removed_updates = updates.iter().filter(|msg| !query.contains(*msg)).cloned().collect::<Vec<_>>();
        let same_updates    = query.iter().filter(|msg| updates.contains(msg)).cloned().collect::<Vec<_>>();

        println!("Waiting for things to settle down again");
        context.wait_for_idle(100).await;
        println!();

        println!("Send test results ({} added, {} removed, {} same)", added_updates.len(), removed_updates.len(), same_updates.len());
        context.send(test_program_id).unwrap()
            .send(TestResult::QueryDifferences { added_updates, removed_updates, same_updates }).await.unwrap();
    }, 1);

    // Test checks that there were only expected differences
    #[derive(Debug)]
    enum TestResult {
        QueryDifferences {
            added_updates:      Vec<SceneUpdate>,
            removed_updates:    Vec<SceneUpdate>,
            same_updates:       Vec<SceneUpdate>,
        }
    }

    impl SceneMessage for TestResult { }

    TestBuilder::new()
        .expect_message(move |result| { 
            match result {
                TestResult::QueryDifferences { added_updates, removed_updates, same_updates } => {
                    println!();
                    println!("======");
                    println!("Same: {}", same_updates.iter().map(|update| format!("{:?}", update)).collect::<Vec<_>>().join("\n    "));
                    println!();
                    println!("Added: {}", added_updates.iter().map(|update| format!("{:?}", update)).collect::<Vec<_>>().join("\n    "));
                    println!();
                    println!("Removed: {}", removed_updates.iter().map(|update| format!("{:?}", update)).collect::<Vec<_>>().join("\n    "));
                    println!("======");
                    println!();

                    let mut added_updates = added_updates;
                    added_updates.retain(|update| {
                        match update {
                            SceneUpdate::Started(program_id, _)         => !program_id.is_subtask(),
                            SceneUpdate::Connected(source, target, _)   => !source.is_subtask() && !target.is_subtask() && !(source == &query_program && target == &*SCENE_CONTROL_PROGRAM),

                            _ => true
                        }
                    });

                    if !added_updates.is_empty() {
                        Err(format!("Query had extra updates: {}", added_updates.iter().map(|update| format!("{:?}", update)).collect::<Vec<_>>().join("\n    ")))
                    } else if !removed_updates.is_empty() {
                        Err(format!("Subscription had extra updates: {}", removed_updates.iter().map(|update| format!("{:?}", update)).collect::<Vec<_>>().join("\n    ")))
                    } else {
                        Ok(())
                    }
                }
            }
        })
        .run_in_scene_with_threads(&scene, test_program_id, 5);
}

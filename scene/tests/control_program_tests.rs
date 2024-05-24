//!
//! The control program can be used to start new programs and stop them from within
//! a program in a scene. It's started when a scene is created with `Scene::default()`.
//! It's an optional program, so a scene that does not need to be dynamic or which has
//! its own method of controlling when it starts and stops can start with `Scene::empty()`
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::future::{select};
use futures::executor;
use futures_timer::*;

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

    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Started(prog_id) => *prog_id == program_1, _ => false }).count() == 1,
        "Program 1 started more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Started(prog_id) => *prog_id == program_2, _ => false }).count() == 1,
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

            context.send_message(subscribe::<SceneUpdate>()).await.unwrap();

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

    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Started(prog_id) => *prog_id == program_1, _ => false }).count() == 1,
        "Program 1 started more than once or didn't start");
    assert!(recv_updates.iter().filter(|item| match item { SceneUpdate::Started(prog_id) => *prog_id == program_2, _ => false }).count() == 1,
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
        .send_message(query::<SceneUpdate>())
        .expect_message_async(move |response: QueryResponse::<SceneUpdate>| async move { 
            let response = response.collect::<Vec<_>>().await;

            if response.is_empty() { return Err("No updates in query response".to_string()); }
            if !response.iter().any(|update| update == &SceneUpdate::Started(program_1)) { return Err(format!("Program 1 ({:?}) not in query response ({:?})", program_1, response)); }
            if !response.iter().any(|update| update == &SceneUpdate::Started(*SCENE_CONTROL_PROGRAM)) { return Err(format!("Scene control program not in query response ({:?})", response)); }

            Ok(()) 
        })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

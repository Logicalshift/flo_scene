//!
//! The idle request program is used to notify when a scene has become idle, which is to say
//! that it has processed all of the messages that have been sent and is waiting for new ones
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::future;

use serde::*;

#[test]
fn notify_on_idle() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn notifies_if_subprogram_drops_input_stream() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    // This will drop the input stream before processing any messages, will happen a lot when we create subprograms that don't process any input
    scene.add_subprogram(SubProgramId::new(), |_: InputStream<()>, _| async move {
        future::pending::<()>().await;
    }, 0);

    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn wait_for_idle_then_send_message_empty_scene() {
    let scene           = Scene::empty();
    let test_program    = SubProgramId::new();

    scene.add_subprogram(SubProgramId::new(), move |_input: InputStream<()>, context| async move {
        context.wait_for_idle(1000).await;

        context.send(test_program).unwrap()
            .send(IdleNotification).await.unwrap();
    }, 1);

    TestBuilder::new()
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn wait_for_idle_then_send_message_default_scene() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    scene.add_subprogram(SubProgramId::new(), move |_input: InputStream<()>, context| async move {
        context.wait_for_idle(1000).await;

        context.send(test_program).unwrap()
            .send(IdleNotification).await.unwrap();
    }, 1);

    TestBuilder::new()
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn wait_for_idle_program_errors_when_full() {
    let scene           = Scene::empty();
    let test_program    = SubProgramId::new();
    let waiting_program = SubProgramId::new();

    #[derive(PartialEq, Debug, Serialize, Deserialize)]
    struct TrySend;
    impl SceneMessage for TrySend { }

    #[derive(Debug, Serialize, Deserialize)]
    struct SendResult(Result<(), SceneSendError<TrySend>>);
    impl SceneMessage for SendResult { }

    scene.add_subprogram(SubProgramId::new(), move |_: InputStream<()>, context| async move {
        let mut idle_program = context.send(waiting_program).unwrap();

        // The input stream is asleep so we can send one message to wake it up
        let wakes_the_queue = idle_program.send(TrySend).await;
        assert!(wakes_the_queue.is_ok(), "Should have woken the stream: {:?}", wakes_the_queue);

        // Try to send the message, and then send the result to the test program
        let should_be_error = idle_program.send(TrySend).await;
        context.send(test_program).unwrap().send(SendResult(should_be_error)).await.unwrap();
    }, 0);

    // Add a program that waits for idle but has a 0 length waiting queue
    scene.add_subprogram(waiting_program, move |input: InputStream<TrySend>, context| async move {
        use std::mem;

        context.wait_for_idle(0).await;

        context.send(test_program).unwrap()
            .send(IdleNotification).await.unwrap();

        // If we drop the input early we'll just reject everything, so make sure that the input is dropped last
        mem::drop(input);
    }, 0);

    TestBuilder::new()
        .expect_message(|SendResult(msg)| { if msg != Err(SceneSendError::CannotAcceptMoreInputUntilSceneIsIdle(TrySend)) { Err(format!("Expected error, got {:?}", msg)) } else { Ok(()) } })
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn wait_for_idle_program_closed_input_stream() {
    let scene           = Scene::empty();
    let test_program    = SubProgramId::new();
    let waiting_program = SubProgramId::new();

    #[derive(PartialEq, Debug, Serialize, Deserialize)]
    struct TrySend;
    impl SceneMessage for TrySend { }

    #[derive(Debug, Serialize, Deserialize)]
    struct SendResult(Result<(), SceneSendError<TrySend>>);
    impl SceneMessage for SendResult { }

    scene.add_subprogram(SubProgramId::new(), move |_: InputStream<()>, context| async move {
        let mut idle_program = context.send(waiting_program).unwrap();

        // Might wake the queue if the input stream is not closed here
        let maybe_wakes_the_queue = idle_program.send(TrySend).await;

        // A second message should always be an error (indicating the stream is closed)
        let should_be_error = if maybe_wakes_the_queue.is_err() {
            maybe_wakes_the_queue
        } else {
            idle_program.send(TrySend).await
        };

        context.send(test_program).unwrap().send(SendResult(should_be_error)).await.unwrap();
    }, 0);

    // Add a program that waits for idle but has a 0 length waiting queue
    scene.add_subprogram(waiting_program, move |input: InputStream<TrySend>, context| async move {
        // Drop the input stream immediately
        use std::mem;
        mem::drop(input);

        context.wait_for_idle(0).await;

        context.send(test_program).unwrap()
            .send(IdleNotification).await.unwrap();
    }, 0);

    TestBuilder::new()
        .expect_message(|SendResult(msg)| { if msg != Err(SceneSendError::StreamClosed(TrySend)) { Err(format!("Expected error, got {:?}", msg)) } else { Ok(()) } })
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn wait_for_idle_program_errors_after_filling_available_space() {
    let scene           = Scene::empty();
    let test_program    = SubProgramId::new();
    let waiting_program = SubProgramId::new();

    #[derive(PartialEq, Debug, Serialize, Deserialize)]
    struct TrySend;
    impl SceneMessage for TrySend { }

    #[derive(Debug, Serialize, Deserialize)]
    struct SendResult(Result<(), SceneSendError<TrySend>>);
    impl SceneMessage for SendResult { }

    scene.add_subprogram(SubProgramId::new(), move |_: InputStream<()>, context| async move {
        let mut idle_program = context.send(waiting_program).unwrap();

        // Send some messages to fill up the queue (which is set to size 4 below: one extra message because the message that wakes the stream is not queued but blocked instead)
        for i in 0..5 {
            println!("Sending {}", i);
            idle_program.send(TrySend).await.unwrap();
            println!("  Sent {}", i);
        }

        // Try to send the message, and then send the result to the test program
        let should_be_error = idle_program.send(TrySend).await;
        context.send(test_program).unwrap().send(SendResult(should_be_error)).await.unwrap();
    }, 0);

    // Add a program that waits for idle but has a 0 length waiting queue
    scene.add_subprogram(waiting_program, move |input: InputStream<TrySend>, context| async move {
        use std::mem;

        context.wait_for_idle(4).await;

        context.send(test_program).unwrap()
            .send(IdleNotification).await.unwrap();

        // If we drop the input early we'll just reject everything
        mem::drop(input);
    }, 2);

    TestBuilder::new()
        .expect_message(|SendResult(msg)| { if msg != Err(SceneSendError::CannotAcceptMoreInputUntilSceneIsIdle(TrySend)) { Err(format!("Expected error, got {:?}", msg)) } else { Ok(()) } })
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn wait_for_idle_program_queues_extra_requests() {
    let scene           = Scene::empty();
    let test_program    = SubProgramId::new();
    let waiting_program = SubProgramId::new();

    #[derive(PartialEq, Debug, Serialize, Deserialize)]
    struct TrySend;
    impl SceneMessage for TrySend { }

    scene.add_subprogram(SubProgramId::new(), move |_: InputStream<()>, context| async move {
        let mut idle_program = context.send(waiting_program).unwrap();

        // Send some messages to it (these get blocked while 'wait_for_idle' is waiting)
        for _ in 0..5 {
            idle_program.send(TrySend).await.unwrap();
        }
    }, 0);

    // Add a program that waits for idle but has a 0 length waiting queue
    scene.add_subprogram(waiting_program, move |input: InputStream<TrySend>, context| async move {
        // Wait for idle, then tell the test program we're OK
        context.wait_for_idle(1_000).await;

        context.send(test_program).unwrap()
            .send(IdleNotification).await.unwrap();

        // Wait for idle so we know the earlier message has been delivered
        // TODO: figure out a way to change filtering so that re-ordering can't happen
        context.wait_for_idle(100).await;

        // Forward the 5 messages
        let mut input = input;
        for _ in 0..5 {
            context.send(test_program).unwrap()
                .send(input.next().await.unwrap()).await.unwrap();
        }
    }, 0);

    TestBuilder::new()
        .expect_message(|IdleNotification| { Ok(()) })
        .expect_message(|TrySend| { Ok(()) })
        .expect_message(|TrySend| { Ok(()) })
        .expect_message(|TrySend| { Ok(()) })
        .expect_message(|TrySend| { Ok(()) })
        .expect_message(|TrySend| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn wait_for_idle_program_queues_extra_requests_100_times() {
    for _ in 0..100 {
        let scene           = Scene::empty();
        let test_program    = SubProgramId::new();
        let waiting_program = SubProgramId::new();

        #[derive(PartialEq, Debug, Serialize, Deserialize)]
        struct TrySend;
        impl SceneMessage for TrySend { }

        scene.add_subprogram(SubProgramId::new(), move |_: InputStream<()>, context| async move {
            let mut idle_program = context.send(waiting_program).unwrap();

            // Send some messages to it (these get blocked while 'wait_for_idle' is waiting)
            for _ in 0..5 {
                idle_program.send(TrySend).await.unwrap();
            }
        }, 0);

        // Add a program that waits for idle but has a 0 length waiting queue
        scene.add_subprogram(waiting_program, move |input: InputStream<TrySend>, context| async move {
            // Wait for idle, then tell the test program we're OK
            context.wait_for_idle(1_000).await;

            context.send(test_program).unwrap()
                .send(IdleNotification).await.unwrap();

            // Wait for idle so we know the earlier message has been delivered
            // TODO: figure out a way to change filtering so that re-ordering can't happen
            context.wait_for_idle(100).await;

            // Forward the 5 messages
            let mut input = input;
            for _ in 0..5 {
                context.send(test_program).unwrap()
                    .send(input.next().await.unwrap()).await.unwrap();
            }
        }, 0);

        TestBuilder::new()
            .expect_message(|IdleNotification| { Ok(()) })
            .expect_message(|TrySend| { Ok(()) })
            .expect_message(|TrySend| { Ok(()) })
            .expect_message(|TrySend| { Ok(()) })
            .expect_message(|TrySend| { Ok(()) })
            .expect_message(|TrySend| { Ok(()) })
            .run_in_scene_with_threads(&scene, test_program, 5);
    }
}

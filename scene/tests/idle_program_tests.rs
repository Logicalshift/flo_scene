//!
//! The idle request program is used to notify when a scene has become idle, which is to say
//! that it has processed all of the messages that have been sent and is waiting for new ones
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::future;

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
fn wait_for_idle_then_send_message() {
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
fn wait_for_idle_program_errors_when_full() {
    let scene           = Scene::empty();
    let test_program    = SubProgramId::new();
    let waiting_program = SubProgramId::new();

    #[derive(PartialEq, Debug)]
    struct TrySend;
    impl SceneMessage for TrySend { }

    #[derive(Debug)]
    struct SendResult(Result<(), SceneSendError<TrySend>>);
    impl SceneMessage for SendResult { }

    scene.add_subprogram(SubProgramId::new(), move |_: InputStream<()>, context| async move {
        let mut idle_program = context.send(waiting_program).unwrap();

        // Try to send the message, and then send the result to the test program
        let should_be_error = idle_program.send(TrySend).await;
        context.send(test_program).unwrap().send(SendResult(should_be_error)).await.unwrap();
    }, 0);

    // Add a program that waits for idle but has a 0 length waiting queue
    scene.add_subprogram(waiting_program, move |_input: InputStream<TrySend>, context| async move {
        context.wait_for_idle(0).await;

        context.send(test_program).unwrap()
            .send(IdleNotification).await.unwrap();
    }, 0);

    TestBuilder::new()
        .expect_message(|SendResult(msg)| { if msg != Err(SceneSendError::CannotAcceptMoreInputUntilSceneIsIdle(TrySend)) { Err(format!("Expected error, got {:?}", msg)) } else { Ok(()) } })
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

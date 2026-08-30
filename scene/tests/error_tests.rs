//!
//! The error program provides a standard way to report errors (non-fatal or fatal)
//! to the scene, and a way for other programs to listen to and potentially respond
//! to errors.
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;

#[test]
fn notify_on_error() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    let error_program   = SubProgramId::new();

    scene.add_subprogram(error_program, 
        move |_: InputStream<()>, context| async move {
            // Subscribe the test program to receive errors
            context.send_message(ErrorSubscription::SubscribeToAll(test_program.into())).or_fail().await;

            // Try an 'or error' message
            async { Result::<(), String>::Err(format!("Goodbye, world")) }.with_report().await.ok();
        }, 0);

    TestBuilder::new()
        .expect_message_matching(Error::Error { source: error_program, message: "\"Goodbye, world\"".into() }, "Was expecting an error message from our subprogram")
        .expect_running_scene()
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn notify_on_failure() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    let error_program   = SubProgramId::new();
    let relay_program   = SubProgramId::new();

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestMessage(Error);

    impl SceneMessage for TestMessage { }

    // or_fail() shuts the scene down but waits for it to become idle, so we can still receive the message that the error occurred
    scene.add_subprogram(error_program, 
        move |_: InputStream<()>, context| async move {
            // Subscribe the relay program to receive errors
            context.send_message(ErrorSubscription::SubscribeToAll(relay_program.into())).or_fail().await;

            // Try an 'or error' message
            async { Result::<(), String>::Err(format!("Goodbye, world")) }.or_fail().await;
        }, 0);

    // Relay program sends errors to the test builder
    scene.add_subprogram(relay_program,
        move |input, context| async move {
            let mut input = input;
            while let Some(error) = input.next().await {
                context.send_message(TestMessage(error)).await.ok();
            }
        }, 5);

    TestBuilder::new()
        .expect_message_matching(TestMessage(Error::Failure { source: error_program, message: "\"Goodbye, world\"".into() }), "Was expecting an error message from our subprogram")
        .expect_stopped_scene()
        .run_in_scene_with_threads(&scene, test_program, 5);
}

//!
//! The error program provides a standard way to report errors (non-fatal or fatal)
//! to the scene, and a way for other programs to listen to and potentially respond
//! to errors.
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::executor;
use futures::future::{select};
use futures_timer::*;
use serde::*;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration};

#[test]
fn notify_on_error() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    let error_program   = SubProgramId::called("test_error");
    let relay_program   = SubProgramId::called("test_relay");

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestMessage(Error);

    impl SceneMessage for TestMessage { }

    // Error_program throws an error
    scene.add_subprogram(error_program, 
        move |_: InputStream<()>, context| async move {
            // Subscribe the test program to receive errors
            context.send_message(ErrorSubscription::SubscribeToAll(relay_program.into())).or_fail("Test").await;

            // Try an 'or error' message
            async { Result::<(), String>::Err(format!("Goodbye, world")) }.with_report("Test").await.ok();
        }, 0);

    // Relay program passes any generated errors on to the main program
    scene.add_subprogram(relay_program,
        move |input, context| async move {
            let mut input = input;
            while let Some(error) = input.next().await {
                context.send_message(TestMessage(error)).await.ok();
            }
        }, 5);

    TestBuilder::new()
        .expect_message_matching(TestMessage(Error::Error { source: error_program, message: "Test: \"Goodbye, world\"".into() }), "Was expecting an error message from our subprogram")
        .expect_running_scene()
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn notify_on_failure() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    let error_program   = SubProgramId::called("test_error");
    let relay_program   = SubProgramId::called("test_relay");

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestMessage(Error);

    impl SceneMessage for TestMessage { }

    // or_fail() shuts the scene down but waits for it to become idle, so we can still receive the message that the error occurred
    scene.add_subprogram(error_program, 
        move |_: InputStream<()>, context| async move {
            // Subscribe the relay program to receive errors
            context.send_message(ErrorSubscription::SubscribeToAll(relay_program.into())).or_fail("Test").await;

            // Try an 'or error' message
            async { Result::<(), String>::Err(format!("Goodbye, world")) }.or_fail("Test").await;
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
        .expect_message_matching(TestMessage(Error::Failure { source: error_program, message: "Test: \"Goodbye, world\"".into() }), "Was expecting an error message from our subprogram")
        .expect_stopped_scene()
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn panic_single_threaded() {
    let scene           = Scene::default();
    let panic_program   = SubProgramId::called("test_panic");
    let other_program   = SubProgramId::called("test_other");

    // This subprogram panics as soon as it's polled
    scene.add_subprogram(panic_program,
        move |_: InputStream<()>, _context| async move {
            panic!("Goodbye, world");
        }, 0);

    // This subprogram would run forever: if the scene doesn't stop as a result of the panic, this test will time out instead of panicking
    scene.add_subprogram(other_program,
        move |input: InputStream<()>, _context| async move {
            let mut input = input;
            while let Some(_) = input.next().await { }
        }, 0);

    let mut timed_out  = false;
    let panic_result    = catch_unwind(AssertUnwindSafe(|| {
        executor::block_on(select(async {
            scene.run_scene().await;
        }.boxed(), async {
            Delay::new(Duration::from_millis(5000)).await;
            timed_out = true;
        }.boxed()));
    }));

    assert!(!timed_out, "Scene did not stop after a subprogram panicked");
    assert!(panic_result.is_err(), "run_scene() should resume panicking once the scene has stopped");

    let panic_payload   = panic_result.unwrap_err();
    let panic_message   = panic_payload.downcast_ref::<String>().cloned()
        .or_else(|| panic_payload.downcast_ref::<&str>().map(|msg| msg.to_string()));

    assert_eq!(panic_message.as_deref(), Some("Goodbye, world"), "Expected the original panic message to be resumed");
}

#[test]
fn panic_multi_threaded() {
    let scene           = Scene::default();
    let panic_program   = SubProgramId::called("test_panic");
    let other_program   = SubProgramId::called("test_other");

    // This subprogram panics as soon as it's polled
    scene.add_subprogram(panic_program,
        move |_: InputStream<()>, _context| async move {
            panic!("Goodbye, world (with threads)");
        }, 0);

    // This subprogram would run forever: if the scene doesn't stop as a result of the panic, this test will time out instead of panicking
    scene.add_subprogram(other_program,
        move |input: InputStream<()>, _context| async move {
            let mut input = input;
            while let Some(_) = input.next().await { }
        }, 0);

    let mut timed_out  = false;
    let panic_result    = catch_unwind(AssertUnwindSafe(|| {
        executor::block_on(select(async {
            scene.run_scene_with_threads(4).await;
        }.boxed(), async {
            Delay::new(Duration::from_millis(5000)).await;
            timed_out = true;
        }.boxed()));
    }));

    assert!(!timed_out, "Scene did not stop after a subprogram panicked");
    assert!(panic_result.is_err(), "run_scene_with_threads() should resume panicking once the scene has stopped");

    let panic_payload   = panic_result.unwrap_err();
    let panic_message   = panic_payload.downcast_ref::<String>().cloned()
        .or_else(|| panic_payload.downcast_ref::<&str>().map(|msg| msg.to_string()));

    assert_eq!(panic_message.as_deref(), Some("Goodbye, world (with threads)"), "Expected the original panic message to be resumed");
}

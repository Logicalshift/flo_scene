//!
//! Immediate mode is a way to send messages via an output sink without needing to
//! use an async method. If the target supports it, it can 'steal' the thread and
//! run immediately. This is particularly useful for things like logging where
//! having the message appear immediately or blocking until it is processed is a
//! desirable feature.
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
fn send_message_with_thread_stealing() {
    let scene = Scene::default();

    // Create some status variables to store the state of the message
    let received_message    = Arc::new(Mutex::new(0));
    let received_immediate  = Arc::new(Mutex::new(0));

    // The receiver program reads messages in immediate mode and sets the 'received_message' flag as soon as the message is received
    let receiver_program            = SubProgramId::new();
    let receiver_program_counter    = Arc::clone(&received_message);

    scene.add_subprogram(receiver_program, 
        move |messages: InputStream<()>, _context| {
            messages.allow_thread_stealing(true);

            async move {
                let mut messages = messages;

                // Increase the counter every time we receive a message
                while let Some(_msg) = messages.next().await {
                    *receiver_program_counter.lock().unwrap() += 1;
                }
            }
        }, 0);

    // The sender program sends messages to the receiver in immediate mode
    let sender_program = SubProgramId::new();

    let receiver_program_counter    = Arc::clone(&received_message);
    let output_counter              = Arc::clone(&received_immediate);

    scene.add_subprogram(sender_program, 
        move |_: InputStream<()>, context| {
            let message_sender = context.send::<()>(receiver_program).unwrap();

            async move {
                // Send some immediate messages
                message_sender.send_immediate(()).unwrap();
                message_sender.send_immediate(()).unwrap();
                message_sender.send_immediate(()).unwrap();

                // Store how many have been processed in the output counter
                *output_counter.lock().unwrap() = *receiver_program_counter.lock().unwrap();

                // Stop the scene once we're done
                context.send_message(SceneControl::StopScene).await.unwrap();
            }
        }, 0);

    // Run the scene
    let mut finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Check it behaved as intended
    assert!(*received_immediate.lock().unwrap() == 3, "Expected to have processed 3 messages immediated (processed: {:?})", *received_immediate.lock().unwrap());
    assert!(finished, "Scene did not finish");
}

#[test]
fn cannot_reenter_existing_program() {
    let scene = Scene::default();

    // The easiest way to generate some reentrancy is to make a program that calls itself
    let reentrant_subprogram = SubProgramId::new();

    scene.add_subprogram(reentrant_subprogram, 
        move |messages: InputStream<()>, context| {
            let send_to_self = context.send::<()>(reentrant_subprogram).unwrap();
            messages.allow_thread_stealing(true);

            async move {
                // First 'send' will fill the target's output, second won't work because it's full
                send_to_self.try_send_immediate(()).unwrap();
                let try_send_error = send_to_self.try_send_immediate(());
                assert!(try_send_error.is_err(), "Try send: {:?}", try_send_error);

                // Can't flush because the program is full
                let flush_err = send_to_self.try_flush_immediate();
                assert!(flush_err == Err(SceneSendError::CannotReEnterTargetProgram), "Try flush: {:?}", flush_err);

                // Can't send immediately either
                let send_error = send_to_self.send_immediate(());
                assert!(send_error == Err(SceneSendError::CannotReEnterTargetProgram), "Send immediate: {:?}", send_error);

                context.send_message(SceneControl::StopScene).await.unwrap();
            }
        },
        0);

    // Run the scene
    let mut finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Check it behaved as intended
    assert!(finished, "Scene did not finish");
}

// TODO: test out that we can park in immediate mode
// TODO: thread stealing with normal 'send.await' message sending should likely also work

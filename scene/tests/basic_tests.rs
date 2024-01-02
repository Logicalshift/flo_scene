use flo_scene::*;

use futures::prelude::*;
use futures::future::{select};
use futures::executor;
use futures_timer::*;

use std::time::{Duration};
use std::sync::*;

#[test]
fn run_subprogram_and_stop_when_scene_is_empty() {
    // Flag to say if the subprogram has run
    let has_run     = Arc::new(Mutex::new(false));

    // Create a scene with just this subprogram in it
    let scene       = Scene::empty();
    let run_flag    = has_run.clone();
    scene.add_subprogram(
        SubProgramId::new(),
        move |_: InputStream<()>, _| async move {
            // Set the flag
            *run_flag.lock().unwrap() = true;
        },
        0,
    );

    let mut has_stopped = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        has_stopped = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have set the flag and then finished
    assert!(*has_run.lock().unwrap() == true, "Test program did not run");

    // Should have stopped the scene and not just timed out
    assert!(has_stopped, "Scene did not stop when all the subprograms finished");
}

#[test]
fn send_output_to_subprogram_directly() {
    // Flag to say if the subprogram has run
    let sent_message    = Arc::new(Mutex::new(None));

    // Create a scene with two subprograms. Program_1 will send to Program_2
    let scene       = Scene::empty();
    let program_1   = SubProgramId::new();
    let program_2   = SubProgramId::new();

    // program_1 reads from its input and sets it in sent_message
    let recv_message = sent_message.clone();
    scene.add_subprogram(program_1.clone(),
        move |mut input: InputStream<usize>, _| async move {
            // Read a single message and write it to the 'sent_message' structure
            let message = input.next().await.unwrap();
            *recv_message.lock().unwrap() = Some(message);
        },
        0);

    // program_2 sends a message to program_1 directly (by requesting a stream for program_1)
    scene.add_subprogram(program_2,
        move |_: InputStream<()>, context| async move {
            let mut send_usize = context.send::<usize>(program_1).unwrap();
            send_usize.send(42).await.ok().unwrap();
        },
        0);

    // Run this scene
    executor::block_on(select(async {
        scene.run_scene().await;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have set the flag and then finished
    assert!(*sent_message.lock().unwrap() == Some(42), "Message was not sent");
}

#[test]
fn send_output_to_subprogram_via_all_connection() {
    // Flag to say if the subprogram has run
    let sent_message    = Arc::new(Mutex::new(None));

    // Create a scene with two subprograms. Program_1 will send to Program_2
    let scene       = Scene::empty();
    let program_1   = SubProgramId::new();
    let program_2   = SubProgramId::new();

    // program_1 reads from its input and sets it in sent_message
    let recv_message = sent_message.clone();
    scene.add_subprogram(program_1.clone(),
        move |mut input: InputStream<usize>, _| async move {
            // Read a single message and write it to the 'sent_message' structure
            let message = input.next().await.unwrap();
            *recv_message.lock().unwrap() = Some(message);
        },
        0);

    // program_2 sends a to it's usize output
    scene.add_subprogram(program_2.clone(),
        move |_: InputStream<()>, context| async move {
            let mut send_usize = context.send::<usize>(StreamTarget::Any).unwrap();
            send_usize.send(42).await.ok().unwrap();
        },
        0);

    // Connect all usize streams to program_1
    scene.connect_programs(StreamSource::All, program_1, StreamId::with_message_type::<usize>()).unwrap();

    // Run this scene
    executor::block_on(select(async {
        scene.run_scene().await;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have set the flag and then finished
    assert!(*sent_message.lock().unwrap() == Some(42), "Message was not sent");
}

#[test]
fn send_output_to_subprogram_via_specific_connection() {
    // Flag to say if the subprogram has run
    let sent_message    = Arc::new(Mutex::new(None));

    // Create a scene with two subprograms. Program_1 will send to Program_2
    let scene       = Scene::empty();
    let program_1   = SubProgramId::new();
    let program_2   = SubProgramId::new();

    // program_1 reads from its input and sets it in sent_message
    let recv_message = sent_message.clone();
    scene.add_subprogram(program_1.clone(),
        move |mut input: InputStream<usize>, _| async move {
            // Read a single message and write it to the 'sent_message' structure
            let message = input.next().await.unwrap();
            *recv_message.lock().unwrap() = Some(message);
        },
        0);

    // program_2 sends a to it's usize output
    scene.add_subprogram(program_2.clone(),
        move |_: InputStream<()>, context| async move {
            let mut send_usize = context.send::<usize>(StreamTarget::Any).unwrap();
            send_usize.send(42).await.ok().unwrap();
        },
        0);

    // Connect program_2's usize stream to program_1
    scene.connect_programs(program_2, program_1, StreamId::with_message_type::<usize>()).unwrap();

    // Run this scene
    executor::block_on(select(async {
        scene.run_scene().await;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have set the flag and then finished
    assert!(*sent_message.lock().unwrap() == Some(42), "Message was not sent");
}

#[test]
fn retrieve_subprogram_id() {
    // Flag to say if the subprogram has run
    let received_messages = Arc::new(Mutex::new(vec![]));

    // Create a scene with three subprograms. Program 1 will receive messages from 2 and 3
    let scene       = Scene::empty();
    let program_1   = SubProgramId::new();
    let program_2   = SubProgramId::new();
    let program_3   = SubProgramId::new();

    // program 1 reads messages and checks their origin. It expects 4 messages
    let stored_messages = received_messages.clone();
    scene.add_subprogram(program_1,
        move |input: InputStream<String>, _context| async move {
            let mut input = input.messages_with_sources();

            for _ in 0..4 {
                let next = input.next().await.unwrap();
                stored_messages.lock().unwrap().push(next);
            }
        }, 0);

    // program 2 and 3 both send two messages to program 1
    scene.add_subprogram(program_2,
        move |_: InputStream<()>, context| async move {
            let mut target = context.send::<String>(program_1).unwrap();

            target.send("Program 2 message 1".into()).await.ok();
            target.send("Program 2 message 2".into()).await.ok();
        }, 0);

    scene.add_subprogram(program_3,
        move |_: InputStream<()>, context| async move {
            let mut target = context.send::<String>(program_1).unwrap();

            target.send("Program 3 message 1".into()).await.ok();
            target.send("Program 3 message 2".into()).await.ok();
        }, 0);

    // Run this scene
    executor::block_on(select(async {
        scene.run_scene().await;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Check the receieved messages
    let received_messages = received_messages.lock().unwrap();
    assert!(received_messages.len() == 4, "Expected 4 messages to be sent, {:?}", received_messages);
    assert!(received_messages.contains(&(program_2, "Program 2 message 1".into())), "Expected program 2 message 1, {:?}", received_messages);
    assert!(received_messages.contains(&(program_2, "Program 2 message 2".into())), "Expected program 2 message 2, {:?}", received_messages);
    assert!(received_messages.contains(&(program_3, "Program 3 message 1".into())), "Expected program 3 message 1, {:?}", received_messages);
    assert!(received_messages.contains(&(program_3, "Program 3 message 2".into())), "Expected program 3 message 2, {:?}", received_messages);
}

#[test]
fn connect_multiple_prorgams_via_any_connection() {
    // Flag to say if the subprogram has run
    let received_messages = Arc::new(Mutex::new(vec![]));

    // Create a scene with three subprograms. Program 1 will receive messages from 2 and 3
    let scene       = Scene::empty();
    let program_1   = SubProgramId::new();
    let program_2   = SubProgramId::new();
    let program_3   = SubProgramId::new();

    // program 1 reads messages and checks their origin. It expects 4 messages
    let stored_messages = received_messages.clone();
    scene.add_subprogram(program_1,
        move |input: InputStream<String>, _context| async move {
            let mut input = input.messages_with_sources();

            for _ in 0..4 {
                let next = input.next().await.unwrap();
                stored_messages.lock().unwrap().push(next);
            }
        }, 0);

    // program 2 and 3 both send two messages to program 1
    scene.add_subprogram(program_2,
        move |_: InputStream<()>, context| async move {
            let mut target = context.send::<String>(StreamTarget::Any).unwrap();

            target.send("Program 2 message 1".into()).await.ok();
            target.send("Program 2 message 2".into()).await.ok();
        }, 0);

    scene.add_subprogram(program_3,
        move |_: InputStream<()>, context| async move {
            let mut target = context.send::<String>(StreamTarget::Any).unwrap();

            target.send("Program 3 message 1".into()).await.ok();
            target.send("Program 3 message 2".into()).await.ok();
        }, 0);

    scene.connect_programs(StreamSource::All, program_1, StreamId::with_message_type::<String>()).unwrap();

    // Run this scene
    executor::block_on(select(async {
        scene.run_scene().await;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Check the receieved messages
    let received_messages = received_messages.lock().unwrap();
    assert!(received_messages.len() == 4, "Expected 4 messages to be sent, {:?}", received_messages);
    assert!(received_messages.contains(&(program_2, "Program 2 message 1".into())), "Expected program 2 message 1, {:?}", received_messages);
    assert!(received_messages.contains(&(program_2, "Program 2 message 2".into())), "Expected program 2 message 2, {:?}", received_messages);
    assert!(received_messages.contains(&(program_3, "Program 3 message 1".into())), "Expected program 3 message 1, {:?}", received_messages);
    assert!(received_messages.contains(&(program_3, "Program 3 message 2".into())), "Expected program 3 message 2, {:?}", received_messages);
}

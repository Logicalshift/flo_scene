use flo_scene::*;
use flo_scene::compositor::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;
use futures_timer::{Delay};

use std::sync::*;
use std::time::{Duration};

#[test]
fn send_either_message() {
    // Default scene
    let scene = Scene::default();

    // Our program will accept either of these messages
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageA(String);
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageB(String);

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub enum TestMessage {
        MessageA(String),
        MessageB(String),
    }

    impl SceneMessage for MessageA { }
    impl SceneMessage for MessageB { }
    impl SceneMessage for TestMessage { }

    // Set up a message program that will decode either message A or message B
    let test_program    = SubProgramId::called("test");
    let message_program = SubProgramId::called("message");

    scene.add_subprogram(message_program, |mut input: InputStream<EitherMessage<MessageA, MessageB>>, context| async move {
        while let Some(msg) = input.next().await {
            match msg {
                EitherMessage::Left(MessageA(msg))  => context.send_message(TestMessage::MessageA(msg)).await.unwrap(),
                EitherMessage::Right(MessageB(msg)) => context.send_message(TestMessage::MessageB(msg)).await.unwrap(),
            }
        }
    }, 0);
    scene.connect_programs((), message_program, StreamId::with_message_type::<MessageA>()).unwrap();
    scene.connect_programs((), message_program, StreamId::with_message_type::<MessageB>()).unwrap();

    // Test: send messages, receive test messages back again
    TestBuilder::new()
        .send_message(MessageA("Hello from over there".into()))
        .expect_message_matching(TestMessage::MessageA("Hello from over there".into()), "failure_message")
        .send_message(MessageB("Hello from over here".into()))
        .expect_message_matching(TestMessage::MessageB("Hello from over here".into()), "failure_message")
        .run_in_scene_with_threads(&scene, test_program, 10);
}

#[test]
fn send_either_message_using_extension() {
    // Default scene
    let scene = Scene::default();

    // Our program will accept either of these messages
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageA(String);
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageB(String);

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub enum TestMessage {
        MessageA(String),
        MessageB(String),
    }

    impl SceneMessage for MessageA { }
    impl SceneMessage for MessageB { }
    impl SceneMessage for TestMessage { }

    // Set up a message program that will decode either message A or message B, using the 'with_extra_message_handler' routine to add to an existing program
    let test_program    = SubProgramId::called("test");
    let message_program = SubProgramId::called("message");

    scene.add_subprogram(message_program, (|mut input: InputStream<MessageA>, context: SceneContext| async move {
        while let Some(msg) = input.next().await {
            context.send_message(TestMessage::MessageA(msg.0)).await.unwrap();
        }
    }).with_extra_message_handler(|mut input: InputStream<MessageB>, context, _forwarder| async move {
        while let Some(msg) = input.next().await {
            context.send_message(TestMessage::MessageB(msg.0)).await.unwrap();
        }
    }), 0);
    scene.connect_programs((), message_program, StreamId::with_message_type::<MessageA>()).unwrap();
    scene.connect_programs((), message_program, StreamId::with_message_type::<MessageB>()).unwrap();

    // Test: send messages, receive test messages back again
    TestBuilder::new()
        .send_message(MessageA("Hello from over there".into()))
        .expect_message_matching(TestMessage::MessageA("Hello from over there".into()), "failure_message")
        .send_message(MessageB("Hello from over here".into()))
        .expect_message_matching(TestMessage::MessageB("Hello from over here".into()), "failure_message")
        .run_in_scene_with_threads(&scene, test_program, 10);
}

#[test]
fn extension_is_not_idle_while_messages_are_waiting() {
    // Default scene
    let scene = Scene::default();

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageA(String);
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageB(String);
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Events(Vec<String>);

    impl SceneMessage for MessageA { }
    impl SceneMessage for MessageB { }
    impl SceneMessage for Events { }

    let test_program    = SubProgramId::called("test");
    let message_program = SubProgramId::called("message");
    let sender_program  = SubProgramId::called("sender");
    let events          = Arc::new(Mutex::new(vec![]));

    // The message program waits on another thread for each message, so the scene has nothing to run but still has messages to process
    let program_events = events.clone();
    scene.add_subprogram(message_program, (move |mut input: InputStream<MessageA>, _context: SceneContext| async move {
        while let Some(msg) = input.next().await {
            Delay::new(Duration::from_millis(50)).await;

            program_events.lock().unwrap().push(msg.0);
        }
    }).with_extra_message_handler(|mut input: InputStream<MessageB>, _context, _forwarder| async move {
        while let Some(_) = input.next().await { }
    }), 0);
    scene.connect_programs((), message_program, StreamId::with_message_type::<MessageA>()).unwrap();
    scene.connect_programs((), message_program, StreamId::with_message_type::<MessageB>()).unwrap();

    // The sender sends two messages then waits for idle: both messages should be processed before the scene becomes idle
    let sender_events = events.clone();
    scene.add_subprogram(sender_program, move |_input: InputStream<()>, context| async move {
        context.send_message(MessageA("1".into())).await.unwrap();
        context.send_message(MessageA("2".into())).await.unwrap();

        context.wait_for_idle(100).await;
        sender_events.lock().unwrap().push("idle".into());

        let events = sender_events.lock().unwrap().clone();
        context.send(test_program).unwrap().send(Events(events)).await.unwrap();
    }, 0);

    TestBuilder::new()
        .expect_message_matching(Events(vec!["1".into(), "2".into(), "idle".into()]), "Scene became idle before all messages were processed")
        .run_in_scene_with_threads(&scene, test_program, 10);
}

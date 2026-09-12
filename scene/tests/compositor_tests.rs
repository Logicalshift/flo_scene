use flo_scene::*;
use flo_scene::compositor::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;

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

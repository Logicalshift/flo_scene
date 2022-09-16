use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::channel::mpsc;

#[derive(Debug, PartialEq)]
enum TestMessage {
    StringValue(String),
}

impl From<String> for TestMessage {
    fn from(str: String) -> TestMessage {
        TestMessage::StringValue(str)
    }
}

impl Into<String> for TestMessage {
    fn into(self) -> String {
        match self {
            TestMessage::StringValue(value)     => value,
        }
    }
}

impl From<i32> for TestMessage {
    fn from(val: i32) -> TestMessage {
        TestMessage::StringValue(val.to_string())
    }
}
impl From<u64> for TestMessage {
    fn from(val: u64) -> TestMessage {
        TestMessage::StringValue(val.to_string())
    }
}

#[test]
fn convert_message_from_string() {
    let scene                       = Scene::empty();
    let entity_id                   = EntityId::new();
    let (string_send, string_recv)  = mpsc::channel(10);

    // Create an entity that responds to TestMessages
    scene.create_entity(entity_id, |_context, mut msg| async move {
        let mut string_send = string_send;

        while let Some(msg) = msg.next().await {
            let msg: TestMessage = msg;

            match &msg {
                TestMessage::StringValue(str)   => { let str = str.clone(); string_send.try_send(str).unwrap(); },
            }
        }
    }).unwrap();

    // Allow test messages to be received as strings
    scene.convert_message::<i32, TestMessage>().unwrap();
    scene.convert_message::<String, TestMessage>().unwrap();
    scene.convert_message::<u64, TestMessage>().unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        let mut string_recv = string_recv;

        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Send 'Hello' as a string to the entity we just created (this is possible because of the call to scene.convert_message())
            scene_send(entity_id, "Hello".to_string()).await.unwrap();
            let response = string_recv.next().await;

            // Wait for the response
            msg.send(
                (response == Some("Hello".to_string())).into()
            ).await.unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn convert_message_from_number() {
    let scene                       = Scene::empty();
    let entity_id                   = EntityId::new();
    let (string_send, string_recv)  = mpsc::channel(10);

    // Create an entity that responds to TestMessages
    scene.create_entity(entity_id, |_context, mut msg| async move {
        let mut string_send = string_send;

        while let Some(msg) = msg.next().await {
            let msg: TestMessage = msg;

            match &msg {
                TestMessage::StringValue(str)   => { let str = str.clone(); string_send.try_send(str).unwrap(); },
            }
        }
    }).unwrap();

    // Allow test messages to be received as strings
    scene.convert_message::<i32, TestMessage>().unwrap();
    scene.convert_message::<String, TestMessage>().unwrap();
    scene.convert_message::<u64, TestMessage>().unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        let mut string_recv = string_recv;

        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Send 'Hello' as a string to the entity we just created (this is possible because of the call to scene.convert_message())
            scene_send(entity_id, 42u64).await.unwrap();
            let response = string_recv.next().await;

            // Wait for the response
            msg.send(
                (response == Some("42".to_string())).into()
            ).await.unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

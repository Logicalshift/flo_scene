use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;

#[derive(Debug, PartialEq)]
enum TestMessage {
    StringValue(String),
    ReceivedString(String),
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
            TestMessage::ReceivedString(str)    => format!("ReceivedString(\"{}\")", str),
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
    let scene           = Scene::empty();
    let entity_id       = EntityId::new();

    // Create an entity that responds to TestMessages
    scene.create_entity(entity_id, |mut msg| async move {
        while let Some(msg) = msg.next().await {
            let msg: Message<TestMessage, TestMessage> = msg;

            match &*msg {
                TestMessage::StringValue(str)   => { let str = str.clone(); msg.respond(TestMessage::ReceivedString(str)).unwrap(); },
                _                               => {}
            }
        }
    }).unwrap();

    // Allow test messages to be received as strings
    scene.convert_message::<i32, TestMessage>().unwrap();
    scene.convert_message::<String, TestMessage>().unwrap();
    scene.convert_message::<u64, TestMessage>().unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Send 'Hello' as a string to the entity we just created (this is possible because of the call to scene.convert_message())
            let response: TestMessage = scene_send(entity_id, "Hello".to_string()).await.unwrap();

            // Wait for the response
            msg.respond(vec![
                (response == TestMessage::ReceivedString("Hello".to_string())).into()
            ]).unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn convert_message_from_number() {
    let scene           = Scene::empty();
    let entity_id       = EntityId::new();

    // Create an entity that responds to TestMessages
    scene.create_entity(entity_id, |mut msg| async move {
        while let Some(msg) = msg.next().await {
            let msg: Message<TestMessage, TestMessage> = msg;

            match &*msg {
                TestMessage::StringValue(str)   => { let str = str.clone(); msg.respond(TestMessage::ReceivedString(str)).unwrap(); },
                _                               => {}
            }
        }
    }).unwrap();

    // Allow test messages to be received as strings
    scene.convert_message::<i32, TestMessage>().unwrap();
    scene.convert_message::<String, TestMessage>().unwrap();
    scene.convert_message::<u64, TestMessage>().unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Send 'Hello' as a string to the entity we just created (this is possible because of the call to scene.convert_message())
            let response: TestMessage = scene_send(entity_id, 42u64).await.unwrap();

            // Wait for the response
            msg.respond(vec![
                (response == TestMessage::ReceivedString("42".to_string())).into()
            ]).unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn convert_response_to_string() {
    let scene           = Scene::empty();
    let entity_id       = EntityId::new();

    // Create an entity that responds to TestMessages
    scene.create_entity(entity_id, |mut msg| async move {
        while let Some(msg) = msg.next().await {
            let msg: Message<TestMessage, TestMessage> = msg;

            match &*msg {
                TestMessage::StringValue(str)   => { let str = str.clone(); msg.respond(TestMessage::ReceivedString(str)).unwrap(); },
                _                               => {}
            }
        }
    }).unwrap();

    // Allow test messages to be received as strings
    scene.convert_response::<TestMessage, String>().unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Send 'Hello' as a string to the entity we just created (this is possible because of the call to scene.convert_message())
            let response: String = scene_send(entity_id, TestMessage::StringValue("Hello".to_string())).await.unwrap();

            // Wait for the response
            msg.respond(vec![
                (response == "ReceivedString(\"Hello\")".to_string()).into()
            ]).unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn convert_message_and_response_to_string() {
    let scene           = Scene::empty();
    let entity_id       = EntityId::new();

    // Create an entity that responds to TestMessages
    scene.create_entity(entity_id, |mut msg| async move {
        while let Some(msg) = msg.next().await {
            let msg: Message<TestMessage, TestMessage> = msg;

            match &*msg {
                TestMessage::StringValue(str)   => { let str = str.clone(); msg.respond(TestMessage::ReceivedString(str)).unwrap(); },
                _                               => {}
            }
        }
    }).unwrap();

    // Convert both messages and responses to and from strings
    scene.convert_message::<String, TestMessage>().unwrap();
    scene.convert_response::<TestMessage, String>().unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Send 'Hello' as a string to the entity we just created (this is possible because of the call to scene.convert_message())
            let response: String = scene_send(entity_id, "Hello".to_string()).await.unwrap();

            // Wait for the response, and succeed if the result is 'world'
            msg.respond(vec![
                (response == "ReceivedString(\"Hello\")".to_string()).into()
            ]).unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

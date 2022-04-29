use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::stream;

use std::sync::*;

#[test]
fn say_hello() {
    let scene           = Scene::empty();
    let hello_entity    = EntityId::new();

    // Create an entity that says 'World' in response 'Hello'
    scene.create_entity(hello_entity, |mut msg| async move {
        while let Some(msg) = msg.next().await {
            let msg: Message<String, String> = msg;

            if *msg == "Hello".to_string() {
                msg.respond("World".to_string()).unwrap();
            } else {
                msg.respond("???".to_string()).unwrap();
            }
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Send a 'Hello' message in response
            let world: String = scene_send(hello_entity, "Hello".to_string()).await.unwrap();

            // Wait for the response, and succeed if the result is 'world'
            msg.respond(vec![
                (world == "World".to_string()).into()
            ]).unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn stream_hello() {
    let scene               = Scene::empty();
    let stream_entity       = EntityId::new();
    let streamed_strings    = Arc::new(Mutex::new(vec![]));

    // Create an entity that receives a stream of strings and stores them in streamed_strings
    let store_strings = Arc::clone(&streamed_strings);
    scene.create_stream_entity(stream_entity, move |mut strings| async move {
        while let Some(string) = strings.next().await {
            store_strings.lock().unwrap().push(string);
        }
    }).unwrap();

    // Give it another aspect that returns the strings that were streamed into it (given a blank message)
    scene.create_entity(stream_entity, move |mut messages| async move {
        while let Some(msg) = messages.next().await {
            let msg: Message<(), Vec<String>> = msg;
            let strings = streamed_strings.lock().unwrap().clone();

            msg.respond(strings).ok();
        }
    }).unwrap();

    // Test sends a couple of strings and then reads them back again
    scene.create_entity(TEST_ENTITY, move |mut messages| async move {
        while let Some(msg) = messages.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Stream in some stirngs
            scene_send_stream(stream_entity, stream::iter(vec!["Hello".to_string(), "World".to_string()])).unwrap().await;

            // Read the strings using the 'reader' aspect of the test entity
            let strings: Vec<String> = scene_send(stream_entity, ()).await.unwrap();

            if strings == vec!["Hello".to_string(), "World".to_string()] {
                msg.respond(vec![SceneTestResult::Ok]).unwrap();
            } else {
                msg.respond(vec![SceneTestResult::FailedWithMessage(format!("Strings retrieved: {:?}", strings))]).unwrap();
            }
        }
    }).unwrap();

    test_scene(scene);
}
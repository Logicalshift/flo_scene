use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::stream;
use futures::channel::mpsc;

use std::sync::*;

#[test]
fn say_hello() {
    let scene                           = Scene::empty();
    let hello_entity                    = EntityId::new();
    let (hello_sender, hello_receiver)  = mpsc::channel(1);

    // Create an entity that says 'World' in response 'Hello'
    scene.create_entity(hello_entity, |_context, mut msg| async move {
        let mut hello_sender = hello_sender;

        while let Some(msg) = msg.next().await {
            let msg: String = msg;

            if msg == "Hello".to_string() {
                hello_sender.send("World".to_string()).await.unwrap();
            } else {
                hello_sender.send("???".to_string()).await.unwrap();
            }
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        let mut hello_receiver = hello_receiver;

        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Send a 'Hello' message in response
            scene_send(hello_entity, "Hello".to_string()).await.unwrap();
            let world = hello_receiver.next().await.unwrap();

            // Wait for the response, and succeed if the result is 'world'
            msg.send((world == "World".to_string()).into()).await.unwrap();
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
    let (done, done_recv)   = mpsc::channel(1);

    // Create an entity that receives a stream of strings and stores them in streamed_strings
    let store_strings = Arc::clone(&streamed_strings);
    scene.create_entity(stream_entity, move |_context, mut strings| async move {
        let mut done = done;

        while let Some(string) = strings.next().await {
            store_strings.lock().unwrap().push(string);

            done.send(()).await.ok();
        }
    }).unwrap();

    // Test sends a couple of strings and then reads them back again
    scene.create_entity(TEST_ENTITY, move |_context, mut messages| async move {
        let mut done_recv = done_recv;

        while let Some(msg) = messages.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Stream in some strings
            scene_send_stream(stream_entity, stream::iter(vec!["Hello".to_string(), "World".to_string()])).unwrap().await;

            // Re-read them from the store
            done_recv.next().await;
            done_recv.next().await;
            let strings: Vec<String> = streamed_strings.lock().unwrap().clone();

            if strings == vec!["Hello".to_string(), "World".to_string()] {
                msg.send(SceneTestResult::Ok).await.unwrap();
            } else {
                msg.send(SceneTestResult::FailedWithMessage(format!("Strings retrieved: {:?}", strings))).await.unwrap();
            }
        }
    }).unwrap();

    test_scene(scene);
}

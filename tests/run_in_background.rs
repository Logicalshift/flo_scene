use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::channel::mpsc;

#[test]
fn say_hello_in_background() {
    let scene                                       = Scene::empty();
    let hello_entity                                = EntityId::new();
    let (mut string_sender, mut string_receiver)    = mpsc::channel(5);
    let (mut relay_sender, mut relay_receiver)      = mpsc::channel(5);

    // Create an entity that monitors string_receiver in the background
    scene.create_entity(hello_entity, |_context, mut msg| async move {
        scene_run_in_background(async move {
            while let Some(string) = string_receiver.next().await {
                relay_sender.send(string).await.ok();
            }
        }).unwrap();

        // Messages don't really matter here
        while let Some(msg) = msg.next().await {
            let msg: Message<String, String> = msg;

            msg.respond("???".to_string()).ok();
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Send a message to the background future
            string_sender.send("Hello".to_string()).await.ok();

            // Should receive another message from the receiver
            let received = relay_receiver.next().await;

            // Wait for the response, and succeed if the result is 'world'
            msg.respond(vec![
                (received == Some("Hello".to_string())).into()
            ]).unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

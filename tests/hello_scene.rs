use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;

#[test]
fn say_hello() {
    let scene           = Scene::empty();
    let hello_entity    = EntityId::new();

    // Create an entity that says 'World' in response 'Hello'
    scene.create_entity::<String, String, _, _>(hello_entity, |mut msg| async move {
        while let Some(msg) = msg.next().await {
            if *msg == "Hello".to_string() {
                msg.respond("World".to_string()).unwrap();
            } else {
                msg.respond("???".to_string()).unwrap();
            }
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity::<(), Vec<SceneTestResult>, _, _>(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            // Send a 'Hello' message
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

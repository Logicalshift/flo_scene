use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::channel::mpsc;

#[test]
fn seal_entity() {
    let scene           = Scene::default();
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

            // Open a channel to the entity
            let mut hello_channel   = scene_send_to::<String, String>(hello_entity).unwrap();

            // Seal the entity
            SceneContext::current().seal_entity(hello_entity).unwrap();

            // Should not be able to open a new channel
            let sealed_channel      = scene_send_to::<String, String>(hello_entity);

            // Should still be able to send to the main channel
            let world               = hello_channel.send("Hello".to_string()).await.unwrap();

            // Wait for the response, and succeed if the result is 'world'
            msg.respond(vec![
                (world == "World".to_string()).into(),
                sealed_channel.is_err().into(),
            ]).unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn close_entity() {
    let scene           = Scene::default();
    let hello_entity    = EntityId::new();

    let (send_shutdown, is_shutdown) = mpsc::channel(1);

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

        let mut send_shutdown = send_shutdown;
        send_shutdown.send(()).await.ok();
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        let mut is_shutdown = is_shutdown;

        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Request registry updates
            let (update_registry, registry_updates) = SimpleEntityChannel::new(TEST_ENTITY, 1000);
            scene_send::<_, ()>(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntities(update_registry.boxed())).await.unwrap();

            // Open a channel to the entity
            let mut hello_channel = scene_send_to::<String, String>(hello_entity).unwrap();

            // Seal the entity
            SceneContext::current().close_entity(hello_entity).unwrap();

            // Should no longer be able to send to the main channel
            let world = hello_channel.send("Hello".to_string()).await;

            // 'is_shutdown' should signal
            is_shutdown.next().await;

            // Registry should indicate that the hello was stopped
            let mut registry_updates = registry_updates;
            while let Some(msg) = registry_updates.next().await {
                if *msg == EntityUpdate::DestroyedEntity(hello_entity) {
                    break;
                }
            }

            // Wait for the response, and succeed if the result is 'world'
            msg.respond(vec![
                world.is_err().into(),
            ]).unwrap();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

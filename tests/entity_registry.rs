use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::channel::mpsc;

use std::collections::{HashSet};

#[test]
fn retrieve_existing_entities() {
    let scene           = Scene::default();
    let hello_entity    = EntityId::new();
    let add_one_entity  = EntityId::new();

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

    // Entity that adds one to any number it's sent
    scene.create_entity(add_one_entity, |mut msg| async move {
        while let Some(msg) = msg.next().await {
            let msg: Message<u64, u64> = msg;
            let val = *msg;

            msg.respond(val + 1).unwrap();
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Create an entity to monitor for what exists in the scene
            let (sender, receiver)  = mpsc::channel(100);
            let entity_monitor      = EntityId::new();

            scene_create_stream_entity(entity_monitor, move |mut messages| async move {
                let mut sender = sender;

                while let Some(message) = messages.next().await {
                    if let EntityUpdate::CreatedEntity(entity_id) = message {
                        sender.send(entity_id).await.ok();
                    }
                }
            }).unwrap();

            // Ask the entity registry to monitor the entities in the scene
            scene_send::<_, ()>(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntities(entity_monitor)).await.unwrap();

            // The 'hello_entity' ID should get sent back to us (pre-existing at the time tracking started)
            let mut receiver    = receiver;
            let mut received    = HashSet::new();
            let expected        = vec![hello_entity, add_one_entity, entity_monitor].into_iter().collect::<HashSet<_>>();

            while let Some(entity_id) = receiver.next().await {
                if entity_id == hello_entity || entity_id == add_one_entity || entity_id == entity_monitor {
                    received.insert(entity_id);
                }

                if received == expected {
                    // Success when we get both entities back again
                    msg.respond(vec![SceneTestResult::Ok]).unwrap();
                    break;
                }
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn retrieve_existing_entities_with_type() {
    let scene           = Scene::default();
    let hello_entity    = EntityId::new();
    let add_one_entity  = EntityId::new();

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

    // Entity that adds one to any number it's sent
    scene.create_entity(add_one_entity, |mut msg| async move {
        while let Some(msg) = msg.next().await {
            let msg: Message<u64, u64> = msg;
            let val = *msg;

            msg.respond(val + 1).unwrap();
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Create an entity to monitor for what exists in the scene
            let (sender, receiver)  = mpsc::channel(100);
            let entity_monitor      = EntityId::new();

            scene_create_stream_entity(entity_monitor, move |mut messages| async move {
                let mut sender = sender;

                while let Some(message) = messages.next().await {
                    if let EntityUpdate::CreatedEntity(entity_id) = message {
                        sender.send(entity_id).await.ok();
                    }
                }
            }).unwrap();

            // Ask the entity registry to monitor the entities in the scene
            scene_send::<_, ()>(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntitiesWithType(entity_monitor, EntityChannelType::of::<u64, u64>())).await.unwrap();

            // The 'hello_entity' ID should get sent back to us (pre-existing at the time tracking started)
            let mut receiver = receiver;

            while let Some(entity_id) = receiver.next().await {
                if entity_id != add_one_entity {
                    // Failed if we get any entity other than the add one entity (assuming there's no u64, u64 built-in entity)
                    msg.respond(vec![SceneTestResult::Failed]).unwrap();
                    break;
                }

                if entity_id == add_one_entity {
                    // Success when we get the entity back again
                    msg.respond(vec![SceneTestResult::Ok]).unwrap();
                    break;
                }
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

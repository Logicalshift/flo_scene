use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::channel::mpsc;

use std::collections::{HashSet};

#[test]
fn open_entity_registry_channel() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Try to open the channel to the entity registry entity and ensure that it's there
            let channel = scene_send_to::<EntityRegistryRequest>(ENTITY_REGISTRY);

            if channel.is_ok() {
                msg.send(SceneTestResult::Ok).await.ok();
            } else {
                msg.send(SceneTestResult::FailedWithMessage(format!("{:?}", channel.err()))).await.ok();
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn retrieve_existing_entities() {
    let scene           = Scene::default();
    let hello_entity    = EntityId::new();
    let add_one_entity  = EntityId::new();

    // Create an entity that consumes strings
    scene.create_entity(hello_entity, |_context, mut msg| async move {
        while let Some(msg) = msg.next().await {
            let _msg: String = msg;
        }
    }).unwrap();

    // Entity that adds one to any number it's sent
    scene.create_entity(add_one_entity, |_context, mut msg| async move {
        while let Some(msg) = msg.next().await {
            let _val: u64 = msg;
        }
    }).unwrap();

    // Create a test for this scene
    test_scene_with_recipe(scene, Recipe::new()
        .wait_for_unordered(vec![EntityUpdate::CreatedEntity(hello_entity), EntityUpdate::CreatedEntity(add_one_entity)])
        .after_sending_messages(ENTITY_REGISTRY, |channel| vec![EntityRegistryRequest::TrackEntities(channel)])
    );

    /*
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Create an entity to monitor for what exists in the scene
            let (sender, receiver)  = mpsc::channel(100);
            let entity_monitor      = EntityId::new();

            scene_create_entity(entity_monitor, move |_context, mut messages| async move {
                let mut sender = sender;

                while let Some(message) = messages.next().await {
                    if let EntityUpdate::CreatedEntity(entity_id) = message {
                        sender.send(entity_id).await.ok();
                    }
                }
            }).unwrap();

            // Ask the entity registry to monitor the entities in the scene
            
            // Note this illustrates an interesting problem: the entity is immediately available after create_entity has been called
            // but it might not be initialised. So this TrackEntities message can (and general does) arrive at the entity registry before
            // the CreatedEntity message. Then it tries to send the existing entities, filling up the channel for the new entity and
            // never getting to processing the CreatedEntity message (which is a deadlock as the registry is stuck trying to send
            // messages and the receiver of those messages is waiting for a response to its CreatedEntity request)
            let entity_monitor_channel = scene_send_to(entity_monitor).unwrap();
            scene_send(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntities(entity_monitor_channel)).await.unwrap();

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
                    msg.send(SceneTestResult::Ok).await.unwrap();
                    break;
                }
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
    */
}

#[test]
fn retrieve_existing_entities_with_type() {
    let scene           = Scene::default();
    let hello_entity    = EntityId::new();
    let add_one_entity  = EntityId::new();

    // Create an entity that says 'World' in response 'Hello'
    scene.create_entity(hello_entity, |_context, mut msg| async move {
        while let Some(msg) = msg.next().await {
            let _msg: String = msg;
        }
    }).unwrap();

    // Entity that adds one to any number it's sent
    scene.create_entity(add_one_entity, |_context, mut msg| async move {
        while let Some(msg) = msg.next().await {
            let _val: u64 = msg;
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Create an entity to monitor for what exists in the scene
            let (sender, receiver)  = mpsc::channel(100);
            let entity_monitor      = EntityId::new();

            scene_create_entity(entity_monitor, move |_context, mut messages| async move {
                let mut sender = sender;

                while let Some(message) = messages.next().await {
                    if let EntityUpdate::CreatedEntity(entity_id) = message {
                        sender.send(entity_id).await.ok();
                    }
                }
            }).unwrap();

            // Ask the entity registry to monitor the entities in the scene
            let entity_monitor_channel = scene_send_to(entity_monitor).unwrap();
            scene_send(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntitiesWithType(entity_monitor_channel, EntityChannelType::of::<u64>())).await.unwrap();

            // The 'hello_entity' ID should get sent back to us (pre-existing at the time tracking started)
            let mut receiver = receiver;

            while let Some(entity_id) = receiver.next().await {
                if entity_id != add_one_entity {
                    // Failed if we get any entity other than the add one entity (assuming there's no u64 built-in entity)
                    msg.send(SceneTestResult::Failed).await.unwrap();
                    break;
                }

                if entity_id == add_one_entity {
                    // Success when we get the entity back again
                    msg.send(SceneTestResult::Ok).await.unwrap();
                    break;
                }
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;

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
    test_scene_with_recipe(scene, Recipe::new()
        .wait_for(vec![EntityUpdate::CreatedEntity(add_one_entity)])
        .after_sending_messages(ENTITY_REGISTRY, |channel| vec![EntityRegistryRequest::TrackEntitiesWithType(channel, EntityChannelType::of::<u64>())])
    );
}

use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::executor;
use futures::channel::mpsc;

use std::mem;

#[cfg(feature="properties")] use flo_binding::*;

#[test]
fn send_value_to_sink() {
    let (mut sink, mut stream) = property_stream();

    executor::block_on(async {
        sink.send(2).await.unwrap();
        assert!(stream.next().await == Some(2));
    });
}

#[test]
fn first_value_dropped_if_two_values_sent_to_sink() {
    let (mut sink, mut stream) = property_stream();

    // As a properties stream only reports the latest state, if we send two values without reading one, only the first is read
    executor::block_on(async {
        sink.send(1).await.unwrap();
        sink.send(3).await.unwrap();
        assert!(stream.next().await == Some(3));
    });
}

#[test]
fn sink_error_if_stream_dropped() {
    let (mut sink, stream) = property_stream();

    // Sending to a sink where the stream is dropped 
    executor::block_on(async {
        sink.send(1).await.unwrap();

        mem::drop(stream);
        assert!(sink.send(3).await.is_err());
    });
}

#[test]
#[cfg(feature="properties")]
fn open_channel_i64() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Try to open the channel to the properties entity and ensure that it's there
            let channel         = properties_channel::<i64>(PROPERTIES, &SceneContext::current()).await;
            let same_channel    = properties_channel::<i64>(PROPERTIES, &SceneContext::current()).await;

            if channel.is_ok() && same_channel.is_ok() {
                msg.respond(vec![SceneTestResult::Ok]).ok();
            } else {
                msg.respond(vec![SceneTestResult::FailedWithMessage(format!("{:?}", channel.err()))]).ok();
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn open_channel_string() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Try to open the channel to the properties entity and ensure that it's there
            let channel         = properties_channel::<String>(PROPERTIES, &SceneContext::current()).await;
            let same_channel    = properties_channel::<String>(PROPERTIES, &SceneContext::current()).await;

            if channel.is_ok() && same_channel.is_ok() {
                msg.respond(vec![SceneTestResult::Ok]).ok();
            } else {
                msg.respond(vec![SceneTestResult::FailedWithMessage(format!("{:?}", channel.err()))]).ok();
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn follow_string_property() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Create a channel to the properties object
            let mut channel                         = properties_channel::<String>(PROPERTIES, &SceneContext::current()).await.unwrap();

            // Create a string property
            let (string_sender, string_receiver)    = mpsc::channel(5);
            let (string_sink, string_stream)        = property_stream();
            channel.send_without_waiting(PropertyRequest::CreateProperty(PropertyDefinition::new(TEST_ENTITY, "TestString", string_receiver.boxed()))).await.unwrap();
            channel.send_without_waiting(PropertyRequest::Follow(PropertyReference::new(TEST_ENTITY, "TestString"), string_sink)).await.unwrap();

            // If we send a value to the property, it should show up on the property stream
            let mut string_sender   = string_sender;
            string_sender.send("Test".to_string()).await.unwrap();

            let mut string_stream   = string_stream;
            let set_value           = string_stream.next().await;

            msg.respond(vec![
                (set_value == Some("Test".to_string())).into()
            ]).ok();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn bind_string_property() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Create a string property from a binding
            let binding             = bind("Test".to_string());
            property_create("TestProperty", binding.clone()).await.unwrap();

            // Retrieve the binding for the property we just created
            let value               = property_bind::<String>(TEST_ENTITY, "TestProperty").await.unwrap();
            let initial_value       = value.get();

            // Watch for updates on the bound property
            let mut value_updates   = follow(value.clone());
            let initial_value_again = value_updates.next().await;

            // Update our original binding (which the property is following)
            binding.set("AnotherTest".to_string());

            // Wait for the value to update (as it gets sent via another entity, this is not instant)
            let next_value          = value_updates.next().await;

            // Retrieve the updated value via the binding
            let another_value       = value.get();

            msg.respond(vec![
                (initial_value == "Test".to_string()).into(),
                (initial_value_again == Some("Test".to_string())).into(),
                (next_value == Some("AnotherTest".to_string())).into(),
                (another_value == "AnotherTest".to_string()).into(),
            ]).ok();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn property_unbinds_when_entity_destroyed() {
    let scene = Scene::default();

    struct StopTestEntity;
    let property_entity = EntityId::new();
    scene.create_entity(property_entity, move |_context, mut msg| async move {
        while let Some(msg) = msg.next().await {
            let msg: Message<StopTestEntity, ()> = msg;

            msg.respond(()).ok();
            break;
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Create a string property from a binding
            let binding             = bind("Test".to_string());
            property_create_on_entity(property_entity, "TestProperty", binding.clone()).await.unwrap();

            // Retrieve the binding for the property we just created
            let value               = property_bind::<String>(property_entity, "TestProperty").await.unwrap();
            let initial_value       = value.get();

            // Watch for updates on the bound property
            let mut value_updates   = follow(value.clone());
            let initial_value_again = value_updates.next().await;

            // Update our original binding (which the property is following)
            binding.set("AnotherTest".to_string());

            // Wait for the value to update (as it gets sent via another entity, this is not instant)
            let next_value          = value_updates.next().await;

            // Retrieve the updated value via the binding
            let another_value       = value.get();

            // Watch for entities stopping
            let (stopped_entities, mut stop_receiver) = SimpleEntityChannel::new(TEST_ENTITY, 5);
            context.send::<_, ()>(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntities(stopped_entities.boxed())).await.unwrap();

            // Stop the property entity
            context.send::<_, ()>(property_entity, StopTestEntity).await.unwrap();

            // Wait for it to stop
            while let Some(msg) = stop_receiver.next().await {
                if *msg == EntityUpdate::DestroyedEntity(property_entity) {
                    break;
                }
            }

            // Property should no longer exist in the properties object
            let error_value = property_bind::<String>(property_entity, "TestProperty").await;

            msg.respond(vec![
                (initial_value == "Test".to_string()).into(),
                (initial_value_again == Some("Test".to_string())).into(),
                (next_value == Some("AnotherTest".to_string())).into(),
                (another_value == "AnotherTest".to_string()).into(),
                (error_value.is_err()).into(),
            ]).ok();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

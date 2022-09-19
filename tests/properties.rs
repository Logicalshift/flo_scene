use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::channel::mpsc;

#[cfg(feature="properties")] use flo_binding::*;

#[test]
#[cfg(feature="properties")]
fn open_channel_i64() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Try to open the channel to the properties entity and ensure that it's there
            let channel         = properties_channel::<i64>(PROPERTIES, &SceneContext::current()).await;
            let same_channel    = properties_channel::<i64>(PROPERTIES, &SceneContext::current()).await;

            if channel.is_ok() && same_channel.is_ok() {
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
#[cfg(feature="properties")]
fn open_channel_string() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Try to open the channel to the properties entity and ensure that it's there
            let channel         = properties_channel::<String>(PROPERTIES, &SceneContext::current()).await;
            let same_channel    = properties_channel::<String>(PROPERTIES, &SceneContext::current()).await;

            if channel.is_ok() && same_channel.is_ok() {
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
#[cfg(feature="properties")]
fn follow_string_property() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Create a channel to the properties object
            let mut channel         = properties_channel::<String>(PROPERTIES, &SceneContext::current()).await.unwrap();

            // Create a string property
            let (string_sender, string_receiver)    = mpsc::channel(5);
            channel.send(PropertyRequest::CreateProperty(PropertyDefinition::from_stream(TEST_ENTITY, "TestString", string_receiver.boxed(), "".into()))).await.unwrap();

            let (string_binding, target) = FloatingBinding::new();
            channel.send(PropertyRequest::Get(PropertyReference::new(TEST_ENTITY, "TestString"), target)).await.unwrap();
            let string_binding = string_binding.wait_for_binding().await.unwrap();

            // If we send a value to the property, it should show up on the property stream
            let mut string_stream   = follow(string_binding);
            let _empty_value        = string_stream.next().await;

            let mut string_sender   = string_sender;
            string_sender.send("Test".to_string()).await.unwrap();

            let set_value           = string_stream.next().await;

            msg.send(
                (set_value == Some("Test".to_string())).into()
            ).await.ok();
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
            let SceneTestRequest(mut msg) = msg;

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

            msg.send((initial_value == "Test".to_string()).into()).await.ok();
            msg.send((initial_value_again == Some("Test".to_string())).into()).await.ok();
            msg.send((next_value == Some("AnotherTest".to_string())).into()).await.ok();
            msg.send((another_value == "AnotherTest".to_string()).into()).await.ok();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn bind_char_rope() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Create a string property from a binding
            let binding             = RopeBindingMut::<char, ()>::new();
            rope_create("TestProperty", RopeBinding::from_mutable(&binding)).await.unwrap();

            // Retrieve the binding for the property we just created
            let value               = rope_bind::<char, ()>(TEST_ENTITY, "TestProperty").await.unwrap();
            let initial_value       = value.read_cells(0..value.len()).collect::<Vec<_>>();

            // Watch for updates on the bound property
            let mut value_updates   = value.follow_changes();

            // Update our original binding (which the property is following)
            binding.replace(0..0, vec!['T', 'e', 's', 't']);

            // Wait for the value to update (as it gets sent via another entity, this is not instant)
            let _next_value         = value_updates.next().await;

            // Retrieve the updated value via the binding
            let another_value       = value.read_cells(0..value.len()).collect::<Vec<_>>();

            msg.send((initial_value == vec![]).into()).await.ok();
            msg.send((another_value == vec!['T', 'e', 's', 't']).into()).await.ok();
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
            let _msg: StopTestEntity = msg;

            break;
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

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
            context.send(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntities(stopped_entities.boxed())).await.unwrap();

            // Stop the property entity
            context.send(property_entity, StopTestEntity).await.unwrap();

            // Wait for it to stop
            while let Some(msg) = stop_receiver.next().await {
                if msg == EntityUpdate::DestroyedEntity(property_entity) {
                    break;
                }
            }

            // Property should no longer exist in the properties object
            let error_value = property_bind::<String>(property_entity, "TestProperty").await;

            msg.send((initial_value == "Test".to_string()).into()).await.ok();
            msg.send((initial_value_again == Some("Test".to_string())).into()).await.ok();
            msg.send((next_value == Some("AnotherTest".to_string())).into()).await.ok();
            msg.send((another_value == "AnotherTest".to_string()).into()).await.ok();
            msg.send((error_value.is_err()).into()).await.ok();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn entities_property() {
    let scene = Scene::default();

    let sample_entity = EntityId::new();
    scene.create_entity(sample_entity, |_ctxt, mut messages| async move {
        while let Some(msg) = messages.next().await {
            let _msg: () = msg;
        }
    }).unwrap();

    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            let entities            = rope_bind::<EntityId, ()>(PROPERTIES, "Entities").await.unwrap();
            let mut entity_stream   = entities.follow_changes();

            loop {
                // Check for the test entity in the list (test fails if it's never found/this times out)
                if entities.read_cells(0..entities.len()).any(|entity_id| entity_id == sample_entity) {
                    break;
                }

                // Wait for the entities to update
                entity_stream.next().await;
            }

            msg.send(SceneTestResult::Ok).await.ok();
        }
    }).unwrap();

    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn track_string_property_if_created_first() {
    use std::sync::*;

    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(_msg) = msg;

            // Create a string property from a binding
            let binding             = bind("Test".to_string());
            property_create("TestProperty", binding.clone()).await.unwrap();

            // Retrieve the binding for the property we just created
            let _value              = property_bind::<String>(TEST_ENTITY, "TestProperty").await.unwrap();

            // Request tracking information on the specified property
            let mut property_channel                = properties_channel::<String>(PROPERTIES, &context).await.unwrap();
            let (tracker_channel, track_strings)    = SimpleEntityChannel::new(TEST_ENTITY, 5);

            property_channel.send(PropertyRequest::TrackPropertiesWithName("TestProperty".into(), tracker_channel.boxed())).await.unwrap();

            // Should read the 'TestProperty' we just created
            let mut track_strings = track_strings;
            while let Some(PropertyUpdate::Created(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    break;
                }
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn track_string_property_if_created_later() {
    use std::sync::*;

    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(_msg) = msg;

            // Request tracking information on the specified property
            let mut property_channel                = properties_channel::<String>(PROPERTIES, &context).await.unwrap();
            let (tracker_channel, track_strings)    = SimpleEntityChannel::new(TEST_ENTITY, 5);

            property_channel.send(PropertyRequest::TrackPropertiesWithName("TestProperty".into(), tracker_channel.boxed())).await.unwrap();

            // Create a string property from a binding
            let binding             = bind("Test".to_string());
            property_create("TestProperty", binding.clone()).await.unwrap();

            // Retrieve the binding for the property we just created
            let _value              = property_bind::<String>(TEST_ENTITY, "TestProperty").await.unwrap();

            // Should read the 'TestProperty' we just created
            let mut track_strings = track_strings;
            while let Some(PropertyUpdate::Created(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    break;
                }
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn track_string_property_when_destroyed() {
    use std::sync::*;

    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(_msg) = msg;

            // Request tracking information on the specified property
            let mut property_channel                = properties_channel::<String>(PROPERTIES, &context).await.unwrap();
            let (tracker_channel, track_strings)    = SimpleEntityChannel::new(TEST_ENTITY, 5);

            property_channel.send(PropertyRequest::TrackPropertiesWithName("TestProperty".into(), tracker_channel.boxed())).await.unwrap();

            // Create a string property from a binding
            let binding             = bind("Test".to_string());
            property_create("TestProperty", binding.clone()).await.unwrap();

            // Retrieve the binding for the property we just created
            let _value              = property_bind::<String>(TEST_ENTITY, "TestProperty").await.unwrap();

            // Should read the 'TestProperty' we just created
            let mut track_strings = track_strings;
            while let Some(PropertyUpdate::Created(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    break;
                }
            }

            // Destroy the property
            property_channel.send(PropertyRequest::DestroyProperty(PropertyReference::new(TEST_ENTITY, "TestProperty"))).await.unwrap();

            // This should generate a destroyed event for this property
            let mut destroyed = false;
            while let Some(PropertyUpdate::Destroyed(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    destroyed = true;
                    break;
                }
            }
            assert!(destroyed);
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn track_string_property_when_destroyed_by_overwriting() {
    use std::sync::*;

    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(_msg) = msg;

            // Request tracking information on the specified property
            let mut property_channel                = properties_channel::<String>(PROPERTIES, &context).await.unwrap();
            let (tracker_channel, track_strings)    = SimpleEntityChannel::new(TEST_ENTITY, 5);

            property_channel.send(PropertyRequest::TrackPropertiesWithName("TestProperty".into(), tracker_channel.boxed())).await.unwrap();

            // Create a string property from a binding
            let binding             = bind("Test".to_string());
            property_create("TestProperty", binding.clone()).await.unwrap();

            // Retrieve the binding for the property we just created
            let _value              = property_bind::<String>(TEST_ENTITY, "TestProperty").await.unwrap();

            // Should read the 'TestProperty' we just created
            let mut track_strings = track_strings;
            let mut created = false;
            while let Some(PropertyUpdate::Created(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    created = true;
                    break;
                }
            }
            assert!(created);

            // Destroy the property by replacing it with another
            property_create("TestProperty", bind("Also test")).await.unwrap();

            // This should generate a destroyed event for this property
            let mut destroyed = false;
            while let Some(PropertyUpdate::Destroyed(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    destroyed = true;
                    break;
                }
            }
            assert!(destroyed);

            // The destroyed event should be followed by another 'created' event
            let mut created = false;
            while let Some(PropertyUpdate::Created(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    created = true;
                    break;
                }
            }
            assert!(created);
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn track_string_property_when_entity_destroyed() {
    use std::sync::*;

    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(_msg) = msg;

            // Create an entity to attach the properties to
            let entity_id           = EntityId::new();
            let mut empty_entity    = empty_entity(entity_id, &context).unwrap();

            // Request tracking information on the specified property
            let mut property_channel                = properties_channel::<String>(PROPERTIES, &context).await.unwrap();
            let (tracker_channel, track_strings)    = SimpleEntityChannel::new(entity_id, 5);

            property_channel.send(PropertyRequest::TrackPropertiesWithName("TestProperty".into(), tracker_channel.boxed())).await.unwrap();

            // Create a string property from a binding
            let binding             = bind("Test".to_string());
            property_create_on_entity(entity_id, "TestProperty", binding.clone()).await.unwrap();

            // Retrieve the binding for the property we just created
            let _value              = property_bind::<String>(entity_id, "TestProperty").await.unwrap();

            // Should read the 'TestProperty' we just created
            let mut track_strings = track_strings;
            while let Some(PropertyUpdate::Created(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    assert!(property_reference.owner == entity_id);
                    break;
                }
            }

            // Destroy the entity that owns the property
            empty_entity.send(EmptyRequest::Stop).await.ok();

            // This should generate a destroyed event for this property
            let mut destroyed = false;
            while let Some(PropertyUpdate::Destroyed(property_reference)) = track_strings.next().await {
                if property_reference.name == Arc::new("TestProperty".into()) {
                    destroyed = true;
                    break;
                }
            }
            assert!(destroyed);
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn follow_all_string_properties() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Create a couple of entities
            let first_entity_id     = EntityId::new();
            let second_entity_id    = EntityId::new();
            let mut first_entity    = empty_entity(first_entity_id, &context).unwrap();
            let mut second_entity   = empty_entity(second_entity_id, &context).unwrap();

            // Attach properties to both of them
            let binding_1           = bind("Test 1".to_string());
            let binding_2           = bind("Test 2".to_string());
            property_create_on_entity(first_entity_id, "TestProperty", binding_1.clone()).await.unwrap();
            property_create_on_entity(second_entity_id, "TestProperty", binding_2.clone()).await.unwrap();

            // Follow all TestProperties
            let mut follow_all      = properties_follow_all::<String>(&context, "TestProperty");

            // Should get two messages with the initial values (ordering doesn't matter)
            let initial_1 = follow_all.next().await.unwrap();

            if let FollowAll::NewValue(owner, value) = initial_1 {
                if owner == first_entity_id {
                    msg.send((value == "Test 1".to_string()).into()).await.ok();
                } else if owner == second_entity_id  {
                    msg.send((value == "Test 2".to_string()).into()).await.ok();
                } else {
                    msg.send(SceneTestResult::FailedWithMessage("Was expecting a value for one of our two entities".to_string())).await.ok();
                    return;
                }
            } else {
                msg.send(SceneTestResult::FailedWithMessage("Was expecting a new value".to_string())).await.ok();
                return;
            }

            let initial_2 = follow_all.next().await.unwrap();

            if let FollowAll::NewValue(owner, value) = initial_2 {
                if owner == first_entity_id {
                    msg.send((value == "Test 1".to_string()).into()).await.ok();
                } else if owner == second_entity_id  {
                    msg.send((value == "Test 2".to_string()).into()).await.ok();
                } else {
                    msg.send(SceneTestResult::FailedWithMessage("Was expecting a value for one of our two entities".to_string())).await.ok();
                    return;
                }
            } else {
                msg.send(SceneTestResult::FailedWithMessage("Was expecting a new value".to_string())).await.ok();
                return;
            }

            // Should get responses if the properties are updated
            binding_1.set("New value 1".to_string());
            let new_value_1 = follow_all.next().await.unwrap();
            msg.send((new_value_1 == FollowAll::NewValue(first_entity_id, "New value 1".to_string())).into()).await.ok();

            binding_2.set("New value 2".to_string());
            let new_value_2 = follow_all.next().await.unwrap();
            msg.send((new_value_2 == FollowAll::NewValue(second_entity_id, "New value 2".to_string())).into()).await.ok();

            // Should get responses if the entities are destroyed
            second_entity.send(EmptyRequest::Stop).await.ok();
            let second_entity_stopped = follow_all.next().await.unwrap();
            msg.send((second_entity_stopped == FollowAll::Destroyed(second_entity_id)).into()).await.ok();

            // Only the first binding should update now
            binding_2.set("New value 3".to_string());
            binding_1.set("New value 4".to_string());

            let new_value_3 = follow_all.next().await.unwrap();
            msg.send((new_value_3 == FollowAll::NewValue(first_entity_id, "New value 4".to_string())).into()).await.ok();

            // Destroy the other entity (ensures there isn't a delayed effect from setting the binding)
            first_entity.send(EmptyRequest::Stop).await.ok();
            let first_entity_stopped = follow_all.next().await.unwrap();
            msg.send((first_entity_stopped == FollowAll::Destroyed(first_entity_id)).into()).await.ok();

            // Finished all the tests
            msg.send(SceneTestResult::Ok).await.ok();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="properties")]
fn follow_property_updates() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(_msg) = msg;

            // Request tracking information on the specified property
            let mut property_channel = properties_channel::<String>(PROPERTIES, &context).await.unwrap();

            // Create a string property from a binding
            let binding = bind("Test".to_string());
            property_create("TestProperty", binding.clone()).await.unwrap();

            // Follow updates to that property
            let (tracker_channel, track_updates) = SimpleEntityChannel::new(TEST_ENTITY, 1);
            property_channel.send(PropertyRequest::Follow(PropertyReference::new(TEST_ENTITY, "TestProperty"), tracker_channel.boxed())).await.unwrap();

            // Should immediately update with the value on the property
            let mut track_updates   = track_updates;
            let initial_value       = track_updates.next().await.unwrap();
            assert!(initial_value == "Test".to_string());

            // Send an update
            binding.set("Another value".to_string());

            // Should update with the next value
            let another_value       = track_updates.next().await.unwrap();
            assert!(another_value == "Another value".to_string());

            // Destroy the property
            property_channel.send(PropertyRequest::DestroyProperty(PropertyReference::new(TEST_ENTITY, "TestProperty"))).await.unwrap();

            // Should close the channel
            let close_channel       = track_updates.next().await;
            assert!(close_channel.is_none());
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

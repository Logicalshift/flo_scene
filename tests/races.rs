use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::stream;
use futures::channel::mpsc;

use flo_binding::*;

use std::sync::*;

use std::collections::{HashSet};

#[test]
fn race_stream_completion() {
    // Entities don't consume stream items until they're finished processing
    for i in 0..100 {
        println!();
        println!("*** STREAM COMPLETION ITER {}", i);

        let scene               = Scene::empty();
        let stream_entity       = EntityId::new();
        let streamed_strings    = Arc::new(Mutex::new(vec![]));

        // Create an entity that receives a stream of strings and stores them in streamed_strings
        let store_strings = Arc::clone(&streamed_strings);
        scene.create_stream_entity(stream_entity, StreamEntityResponseStyle::RespondAfterProcessing, move |_context, mut strings| async move {
            while let Some(string) = strings.next().await {
                store_strings.lock().unwrap().push(string);
            }
        }).unwrap();

        // Test sends a couple of strings and then reads them back again
        scene.create_entity(TEST_ENTITY, move |_context, mut messages| async move {
            while let Some(msg) = messages.next().await {
                let msg: Message<(), Vec<SceneTestResult>> = msg;

                // Stream in some stirngs
                scene_send_stream(stream_entity, stream::iter(vec!["Hello".to_string(), "World".to_string()])).unwrap().await;

                // Re-read them from the store
                let strings: Vec<String> = streamed_strings.lock().unwrap().clone();

                if strings == vec!["Hello".to_string(), "World".to_string()] {
                    msg.respond(vec![SceneTestResult::Ok]).unwrap();
                } else {
                    msg.respond(vec![SceneTestResult::FailedWithMessage(format!("Strings retrieved: {:?}", strings))]).unwrap();
                }
            }
        }).unwrap();

        test_scene(scene);

        println!("*** STREAM COMPLETION FINISH ITER {}", i);
    }
}

#[test]
fn race_retrieve_existing_entities() {
    // This test has been known to hang on rare occasions
    for i in 0..1000 {
        println!();
        println!("*** RETRIEVE EXISTING ITER {}", i);

        let scene           = Scene::default();
        let hello_entity    = EntityId::new();
        let add_one_entity  = EntityId::new();

        // Create an entity that says 'World' in response 'Hello'
        println!("  Create hello_entity...");
        scene.create_entity(hello_entity, |_context, mut msg| async move {
            println!("    Hello entity starting");

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
        println!("  Create add_one_entity...");
        scene.create_entity(add_one_entity, |_context, mut msg| async move {
            println!("    Add one entity starting");

            while let Some(msg) = msg.next().await {
                let msg: Message<u64, u64> = msg;
                let val = *msg;

                msg.respond(val + 1).unwrap();
            }
        }).unwrap();

        // Create a test for this scene
        println!("  Create test entity...");
        scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
            println!("    Test entity starting");

            // Whenever a test is requested...
            while let Some(msg) = msg.next().await {
                let msg: Message<(), Vec<SceneTestResult>> = msg;

                println!("  Start test...");

                // Create an entity to monitor for what exists in the scene
                let (sender, receiver)  = mpsc::channel(100);
                let entity_monitor      = EntityId::new();

                println!("  Create sender entity...");
                scene_create_stream_entity(entity_monitor, StreamEntityResponseStyle::default(), move |_context, mut messages| async move {
                    let mut sender = sender;

                    println!("  Sender: waiting for messages");
                    while let Some(message) = messages.next().await {
                        println!(" -- Sending {:?}", message);
                        if let EntityUpdate::CreatedEntity(entity_id) = message {
                            sender.send(entity_id).await.ok();
                        }
                        println!(" -- Sent");
                    }
                }).unwrap();

                // Ask the entity registry to monitor the entities in the scene
                println!("  Request tracking...");
                let entity_monitor_channel = scene_send_to(entity_monitor).unwrap();
                scene_send::<_, ()>(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntities(entity_monitor_channel)).await.unwrap();
                println!("  Tracking requested...");

                // The 'hello_entity' ID should get sent back to us (pre-existing at the time tracking started)
                let mut receiver    = receiver;
                let mut received    = HashSet::new();
                let expected        = vec![hello_entity, add_one_entity, entity_monitor].into_iter().collect::<HashSet<_>>();

                println!("  Main message loop...");
                while let Some(entity_id) = receiver.next().await {
                    println!("  Recieved: {:?}", entity_id);
                    if entity_id == hello_entity || entity_id == add_one_entity || entity_id == entity_monitor {
                        received.insert(entity_id);
                    }
                    println!("  So far: {:?}", received);

                    if received == expected {
                        // Success when we get both entities back again
                        msg.respond(vec![SceneTestResult::Ok]).unwrap();
                        break;
                    }
                }
            }
        }).unwrap();

        // Test the scene we just set up
        println!("  Testing scene...");
        test_scene(scene);

        println!("*** RETRIEVE EXISTING FINISH ITER {}", i);
    }
}

#[test]
fn race_close_entity() {
    for i in 0..1000 {
        println!("*** CLOSE ENTITY ITER {}", i);

        let scene           = Scene::default();
        let hello_entity    = EntityId::new();

        let (send_shutdown, is_shutdown) = mpsc::channel(1);

        // Create an entity that says 'World' in response 'Hello'
        scene.create_entity(hello_entity, |_context, mut msg| async move {
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
        scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
            let mut is_shutdown = is_shutdown;

            // Whenever a test is requested...
            while let Some(msg) = msg.next().await {
                let msg: Message<(), Vec<SceneTestResult>> = msg;

                // Request registry updates
                let (update_registry, registry_updates) = SimpleEntityChannel::new(TEST_ENTITY, 1000);
                scene_send::<_, ()>(ENTITY_REGISTRY, EntityRegistryRequest::TrackEntities(update_registry.boxed())).await.unwrap();

                // Open a channel to the entity
                println!("  Opening channel");
                let mut hello_channel = scene_send_to::<String, String>(hello_entity).unwrap();

                // Close the entity
                println!("  Closing entity");
                SceneContext::current().close_entity(hello_entity).unwrap();

                // Should no longer be able to send to the main channel
                println!("  Sending test message");
                let world = hello_channel.send("Hello".to_string()).await;

                // 'is_shutdown' should signal
                println!("  Receiving shutdown");
                is_shutdown.next().await;

                // Registry should indicate that the hello was stopped
                println!("  Waiting for registry");
                let mut registry_updates = registry_updates;
                while let Some(msg) = registry_updates.next().await {
                    println!("    Registry update");
                    if *msg == EntityUpdate::DestroyedEntity(hello_entity) {
                        println!("    Destroyed our entity");
                        break;
                    }
                }

                // Wait for the response, and succeed if the result is 'world'
                println!("Checking response ({:?})", world);

                msg.respond(vec![
                    world.is_err().into(),
                ]).unwrap();

                println!("Test finished");
            }
        }).unwrap();

        // Test the scene we just set up
        println!("Running scene");
        test_scene(scene);
        println!("Scene complete");

        println!("*** CLOSE ENTITY FINISH ITER {}", i);
    }
}

#[test]
#[cfg(feature="properties")]
fn race_follow_string_property() {
    for i in 1..1000 {
        println!("*** FOLLOW_STRING_PROPERTY ITER {}", i);

        let scene = Scene::default();

        // Create a test for this scene
        scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
            // Whenever a test is requested...
            while let Some(msg) = msg.next().await {
                let msg: Message<(), Vec<SceneTestResult>> = msg;

                // Create a channel to the properties object
                println!("Request properties channel");
                let mut channel                         = properties_channel::<String>(PROPERTIES, &SceneContext::current()).await.unwrap();

                // Create a string property
                println!("Create string sender/sinks");
                let (string_sender, string_receiver)    = mpsc::channel(5);

                println!("Create test entity property");
                channel.send_without_waiting(PropertyRequest::CreateProperty(PropertyDefinition::from_stream(TEST_ENTITY, "TestString", string_receiver.boxed(), "".into()))).await.unwrap();
                println!("Follow test entity property");
                let property_binding = channel.send(PropertyRequest::Get(PropertyReference::new(TEST_ENTITY, "TestString"))).await.unwrap().unwrap();

                // If we send a value to the property, it should show up on the property stream
                println!("Receive initial empty value");
                let mut string_stream   = follow(property_binding);
                let _empty_value        = string_stream.next().await;

                let mut string_sender   = string_sender;
                println!("Send string");
                string_sender.send("Test".to_string()).await.unwrap();

                let set_value           = string_stream.next().await;
                println!("  Received {:?}", set_value);

                msg.respond(vec![
                    (set_value == Some("Test".to_string())).into()
                ]).ok();
            }
        }).unwrap();

        // Test the scene we just set up
        test_scene(scene);

        println!("*** FOLLOW_STRING_PROPERTY FINISH ITER {}", i);
    }
}

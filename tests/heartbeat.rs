use flo_scene::*;
use flo_scene::test::*;

use ::desync::*;
use futures::prelude::*;
use futures::channel::mpsc;

#[test]
fn open_heartbeat_channel() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Try to open the channel to the heartbeat entity and ensure that it's there
            let channel = scene_send_to::<HeartbeatRequest, ()>(HEARTBEAT);

            if channel.is_ok() {
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
fn receive_heartbeat_after_message() {
    let scene               = Scene::default();
    let receive_heartbeat   = EntityId::new();

    // Receive_heartbeat either receives a heartbeat or a message
    #[derive(Copy, Clone, PartialEq, Debug)]
    enum TestRequest {
        Message,
        Heartbeat,
    }

    impl From<Heartbeat> for TestRequest {
        fn from(_: Heartbeat) -> TestRequest {
            TestRequest::Heartbeat
        }
    }

    // Add a converter so the test component can receive heartbeats
    scene.convert_message::<Heartbeat, TestRequest>().ok();

    // Create an entity that forwards its requests to another stream
    let (sender, receiver) = mpsc::channel(100);
    scene.create_entity(receive_heartbeat, |mut msg| async move {
        let mut sender = sender;

        while let Some(msg) = msg.next().await {
            let msg: Message<TestRequest, ()> = msg;

            sender.send(*msg).await.ok();
            msg.respond(()).ok();
        }
    }).unwrap();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |mut msg| async move {
        let mut receiver = Some(receiver);
        let background = Desync::new(());

        println!("Test starting");

        // Ask the heartbeat entity to send heartbeats to our test entity
        scene_send::<_, ()>(HEARTBEAT, HeartbeatRequest::RequestHeartbeat(receive_heartbeat)).await.unwrap();

        println!("Heartbeat requested");

        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            println!("Test message received");

            // Send a message to the test request
            scene_send::<_, ()>(receive_heartbeat, TestRequest::Message).await.unwrap();

            println!("Sent message");

            // The test itself will prevent a heartbeat (as we're processing a message), so run in the background (the background thread needs to consume the receiver)
            let mut receiver = receiver.take().unwrap();
            background.future_desync(|_| async move {
                // Look for a Message -> Heartbeat pattern in the result from the receiver
                let mut received_message = false;
                while let Some(test_request) = receiver.next().await {
                    println!("{:?}", test_request);

                    if test_request == TestRequest::Message {
                        received_message = true;
                    } else if test_request == TestRequest::Heartbeat && received_message {
                        msg.respond(vec![SceneTestResult::Ok]).ok();
                        return;
                    }
                }
            }.boxed()).detach();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

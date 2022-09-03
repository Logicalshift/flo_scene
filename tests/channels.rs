use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::channel::mpsc;

#[test]
fn send_message_before_wait() {
    // Race failure in the entity test seems to occur when the entity messages are sent before 
    let scene               = Scene::empty();
    let stream_entity       = EntityId::new();

    // Create an entity that receives a stream of strings and stores them in streamed_strings
    let (string_sender, string_receiver) = mpsc::channel(100);
    scene.create_stream_entity(stream_entity, StreamEntityResponseStyle::RespondAfterProcessing, move |_context, mut strings| async move {
        let mut string_sender = string_sender;

        // Send a message to the entity before it starts
        scene_send_without_waiting(stream_entity, "Hello".to_string()).await.ok();

        // Should read the message we sent
        while let Some(string) = strings.next().await {
            // Send to the test channel
            string_sender.send(string).await.ok();
        }
    }).unwrap();

    // Test sends a couple of strings and then reads them back again
    let mut string_receiver = Some(string_receiver);

    scene.create_entity(TEST_ENTITY, move |_context, mut messages| async move {
        while let Some(msg) = messages.next().await {
            let SceneTestRequest(msg) = msg;

            let received_string = string_receiver.take().unwrap().next().await;

            if received_string == Some("Hello".to_string()) {
                msg.send_without_waiting(SceneTestResult::Ok).await.unwrap();
            } else {
                msg.send_without_waiting(SceneTestResult::FailedWithMessage(format!("Strings retrieved: {:?}", received_string))).await.unwrap();
            }
        }
    }).unwrap();

    test_scene(scene);
}

use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::stream;

use std::sync::*;

#[test]
fn stream_completion() {
    // Entities don't consume stream items until they're finished processing
    for _ in 0..100 {
        let scene               = Scene::empty();
        let stream_entity       = EntityId::new();
        let streamed_strings    = Arc::new(Mutex::new(vec![]));

        // Create an entity that receives a stream of strings and stores them in streamed_strings
        let store_strings = Arc::clone(&streamed_strings);
        scene.create_stream_entity(stream_entity, move |mut strings| async move {
            while let Some(string) = strings.next().await {
                store_strings.lock().unwrap().push(string);
            }
        }).unwrap();

        // Test sends a couple of strings and then reads them back again
        scene.create_entity(TEST_ENTITY, move |mut messages| async move {
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
    }
}

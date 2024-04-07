use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;

use std::time::{Duration};

#[test]
fn reply_to_message() {
    struct TestMessage(String);
    struct TestReply(String);
    impl SceneMessage for TestMessage { }
    impl SceneMessage for TestReply { }

    let scene           = Scene::default();
    let replier_program = SubProgramId::new();
    let test_program    = SubProgramId::new();

    scene.add_subprogram(replier_program, 
        |input, context| {
            async move { 
                let mut input = input;
                while let Some(TestMessage(msg)) = input.next().await {
                    context.reply_with(TestReply(msg)).await.unwrap();
                }
            }
        }, 0);
    scene.connect_programs((), replier_program, StreamId::with_message_type::<TestMessage>()).unwrap();

    TestBuilder::new()
        .send_message(TestMessage("Test".to_string()))
        .expect_message(|TestReply(msg)| { if &msg != "Test" { Err(format!("Expected 'Test' (got {:?})", msg)) } else { Ok(()) } })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

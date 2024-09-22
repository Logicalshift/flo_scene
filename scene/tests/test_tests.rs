use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;

use serde::*;

#[test]
pub fn simple_ping_test_with_test_builder() {
    #[derive(Debug, Serialize, Deserialize)]
    struct Ping;
    impl SceneMessage for Ping {}

    // Create a default scene
    let scene = Scene::default();

    // Add a ping subprogram that responds to () messages with a 'Ping' response
    let ping_program = SubProgramId::new();
    scene.add_subprogram(ping_program, 
        |input: InputStream<()>, context| async move {
            let mut input = input.messages_with_sources();

            while let Some((program_id, _)) = input.next().await {
                let mut target = context.send(program_id).unwrap();

                target.send(Ping).await.unwrap();
            }
        },
        100);
    scene.connect_programs((), ping_program, StreamId::with_message_type::<()>()).unwrap();

    // Test it using the test builder
    TestBuilder::new()
        .send_message(())
        .expect_message(|_: Ping| { Ok(()) })
        .run_in_scene(&scene, SubProgramId::new());
}

#[test]
pub fn multithreaded_ping() {
    #[derive(Debug, Serialize, Deserialize)]
    struct Ping;
    impl SceneMessage for Ping {}

    // Create a default scene
    let scene = Scene::default();

    // Add a ping subprogram that responds to () messages with a 'Ping' response
    let ping_program = SubProgramId::new();
    scene.add_subprogram(ping_program, 
        |input: InputStream<()>, context| async move {
            let mut input = input.messages_with_sources();

            while let Some((program_id, _)) = input.next().await {
                let mut target = context.send(program_id).unwrap();

                target.send(Ping).await.unwrap();
            }
        },
        100);
    scene.connect_programs((), ping_program, StreamId::with_message_type::<()>()).unwrap();

    // Test it using the test builder
    TestBuilder::new()
        .send_message(())
        .expect_message(|_: Ping| { Ok(()) })
        .run_in_scene_with_threads(&scene, SubProgramId::new(), 5);
}

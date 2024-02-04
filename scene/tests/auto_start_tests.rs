use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;

#[test]
fn auto_start_on_first_message() {
    // Define a message with an initialisation routine that starts the default subprogram
    struct AutoStartMessage;

    impl SceneMessage for AutoStartMessage {
        fn initialise(scene: &Scene) {
            // When the message is initialised, create a program and redirect everything there
            scene.add_subprogram(SubProgramId::called("AutoStart"),
                |mut input_stream: InputStream<AutoStartMessage>, _context| async move {
                    while let Some(_) = input_stream.next().await { }
                }, 0);

            scene.connect_programs((), SubProgramId::called("AutoStart"), StreamId::with_message_type::<AutoStartMessage>()).unwrap();
        }
    }

    // Create a scene, but don't start the 'auto start' program
    let scene = Scene::default();

    // Try sending a message to it (should start up when it's first encountered)
    TestBuilder::new()
        .send_message(AutoStartMessage)
        .send_message(AutoStartMessage)
        .run_in_scene(&scene, SubProgramId::new());
}

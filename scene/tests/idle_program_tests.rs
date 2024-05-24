//!
//! The idle request program is used to notify when a scene has become idle, which is to say
//! that it has processed all of the messages that have been sent and is waiting for new ones
//!

use flo_scene::*;
use flo_scene::programs::*;

#[test]
fn notify_on_idle() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

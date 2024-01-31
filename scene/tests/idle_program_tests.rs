//!
//! The idle request program is used to notify when a scene has become idle, which is to say
//! that it has processed all of the messages that have been sent and is waiting for new ones
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::executor;
use futures::future::{select};
use futures_timer::*;

use std::time::{Duration};

#[test]
fn notify_on_idle() {
    let scene = Scene::default();

    // Program that sends a request for an idle message, and stops the scene when it's received
    let idle_program = SubProgramId::new();
    scene.add_subprogram(idle_program, 
        |input_stream: InputStream<IdleNotification>, context| {
            async move {
                // Request to know when the scene is idle
                context.send_message(IdleRequest::WhenIdle(idle_program)).await.unwrap();

                // Wait for the idle notification to be received
                let mut input_stream = input_stream;
                if let Some(IdleNotification) = input_stream.next().await {
                    context.send_message(SceneControl::StopScene).await.unwrap();
                }
            }
        },
        0);

    // Run this scene
    let mut finished = false;

    executor::block_on(select(async {
        scene.run_scene().await;

        finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // The finished flag is only set if the StopScene message is generated
    assert!(finished, "Idle request was never sent, scene never stopped");
}

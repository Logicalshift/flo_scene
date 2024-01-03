use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::future::{select};
use futures::executor;
use futures_timer::*;

use std::time::{Duration};
use std::sync::*;

#[test]
fn ask_control_to_stop_scene() {
    // The default scene has the 'control' program in it
    let scene       = Scene::default();
    scene.add_subprogram(
        SubProgramId::new(),
        move |input: InputStream<()>, context| async move {
            // Send to the control program
            let mut control_program = context.send::<SceneControl>(StreamTarget::Any).unwrap();

            // Tell it to stop the stream
            control_program.send(SceneControl::StopScene).await;

            // Read from our input forever
            let mut input = input;
            while let Some(_) = input.next().await {

            }
        },
        0,
    );

    let mut has_stopped = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        has_stopped = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have stopped the scene and not just timed out
    assert!(has_stopped, "Scene did not stop");
}

#[test]
fn ask_control_to_start_program() {
    let has_started = Arc::new(Mutex::new(false));

    // The default scene has the 'control' program in it
    let scene           = Scene::default();
    let notify_started  = has_started.clone();
    scene.add_subprogram(
        SubProgramId::new(),
        move |input: InputStream<()>, context| async move {
            // Send to the control program
            let mut control_program = context.send::<SceneControl>(StreamTarget::Any).unwrap();

            // Tell it to start a new program
            control_program.send(SceneControl::start_program(SubProgramId::new(), move |_: InputStream<()>, context| {
                let notify_started = notify_started.clone();
                async move {
                    // Set the flag to indicate the new program started
                    *notify_started.lock().unwrap() = true;

                    // Stop the scene as the test is done
                    context.send::<SceneControl>(StreamTarget::Any).unwrap().send(SceneControl::StopScene).await;
                }
            }, 0)).await;

            // Read from our input forever
            let mut input = input;
            while let Some(_) = input.next().await {

            }
        },
        0,
    );

    let mut has_stopped = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        has_stopped = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have stopped the scene and not just timed out
    assert!(*has_started.lock().unwrap() == true, "Subprogram did not start");
    assert!(has_stopped, "Scene did not stop");
}

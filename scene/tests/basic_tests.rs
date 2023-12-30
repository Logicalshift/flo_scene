use flo_scene::*;

use futures::prelude::*;
use futures::future::{select};
use futures::executor;
use futures_timer::*;

use std::time::{Duration};
use std::sync::*;

#[test]
fn run_subprogram_and_stop_when_scene_is_empty() {
    // Flag to say if the subprogram has run
    let has_run = Arc::new(Mutex::new(false));

    // Create a scene with just this subprogram in it
    let scene = Scene::empty();
    let run_flag = has_run.clone();
    scene.add_subprogram(
        SubProgramId::new(),
        move |_: InputStream<()>, _| async move {
            // Set the flag
            *run_flag.lock().unwrap() = true;
        },
        0,
    );

    let mut has_stopped = false;
    executor::block_on(select(async {
        scene.run_scene().await;

        has_stopped = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Should have set the flag and then finished
    assert!(*has_run.lock().unwrap() == true, "Test program did not run");

    // Should have stopped the scene and not just timed out
    assert!(has_stopped, "Scene did not stop when all the subprograms finished");
}

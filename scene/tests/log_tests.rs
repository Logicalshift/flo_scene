use flo_scene::*;
use flo_scene::programs::*;

#[test]
fn basic_log_info() {
    let scene = Scene::default();

    let test_subprogram = SubProgramId::called("test");
    let log_program     = SubProgramId::called("test log");

    scene.add_subprogram(log_program, 
        |_: InputStream<()>, context| async move {
            context.info("Hello, world");
        }, 
        5);

    TestBuilder::new()
        .expect_message_matching(ErrorOutput::Line("\x1b[0;97mI test log                \x1b[0m | Hello, world".into()), "Log message did not match")
        .run_in_scene_with_threads(&scene, test_subprogram, 10);
}

#[test]
fn log_with_name() {
    let scene = Scene::default();

    let test_subprogram = SubProgramId::called("test");
    let log_program     = SubProgramId::called("test log");

    scene.add_subprogram(log_program, 
        |_: InputStream<()>, context| async move {
            context.i_am("Test program");
            context.wait_for_idle(10).await;
            context.info("Hello, world");
        }, 
        5);

    TestBuilder::new()
        .expect_message_matching(ErrorOutput::Line("\x1b[0;97mI Test program            \x1b[0m | Hello, world".into()), "Log message did not match")
        .run_in_scene_with_threads(&scene, test_subprogram, 10);
}

#[test]
fn log_error() {
    let scene = Scene::default();

    let test_subprogram = SubProgramId::called("test");
    let log_program     = SubProgramId::called("test log");

    scene.add_subprogram(log_program, 
        |_: InputStream<()>, context| async move {
            context.i_am("Test program");
            context.wait_for_idle(10).await;

            async { Result::<(), _>::Err("Oops") }.with_report().await.ok();
        }, 
        5);

    TestBuilder::new()
        .expect_message_matching(ErrorOutput::Line("\x1b[0;31m! Test program            \x1b[0m | \"Oops\"".into()), "Log message did not match")
        .expect_running_scene()
        .run_in_scene_with_threads(&scene, test_subprogram, 10);
}

#[test]
fn log_failure() {
    let scene = Scene::default();

    let test_subprogram = SubProgramId::called("test");
    let log_program     = SubProgramId::called("test log");

    scene.add_subprogram(log_program, 
        |_: InputStream<()>, context| async move {
            context.i_am("Test program");
            context.wait_for_idle(10).await;

            async { Result::<(), _>::Err("Oops") }.or_fail().await;
        }, 
        5);

    TestBuilder::new()
        .expect_message_matching(ErrorOutput::Line("\x1b[0;91m!!Test program            \x1b[0m | \"Oops\"".into()), "Log message did not match")
        .expect_stopped_scene()
        .run_in_scene_with_threads(&scene, test_subprogram, 10);
}

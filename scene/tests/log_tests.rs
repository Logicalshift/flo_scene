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
        .expect_message_matching(ErrorOutput::Line("\x1b[0;97mI test log                \x1b[0m | Hello, world\n".into()), "Log message did not match")
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
        .expect_message_matching(ErrorOutput::Line("\x1b[0;97mI Test program            \x1b[0m | Hello, world\n".into()), "Log message did not match")
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

            async { Result::<(), _>::Err("Oops") }.with_report("Test").await.ok();
        }, 
        5);

    TestBuilder::new()
        .expect_message_matching(ErrorOutput::Line("\x1b[0;31m! Test program            \x1b[0m | Test: \"Oops\"\n".into()), "Log message did not match")
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

            async { Result::<(), _>::Err("Oops") }.or_fail("Test").await;
        }, 
        5);

    TestBuilder::new()
        .expect_message_matching(ErrorOutput::Line("\x1b[1;91m!!Test program            \x1b[0m | Test: \"Oops\"\n".into()), "Log message did not match")
        .expect_stopped_scene()
        .run_in_scene_with_threads(&scene, test_subprogram, 10);
}

#[test]
fn log_failure_immediately() {
    let scene = Scene::default();

    let test_subprogram = SubProgramId::called("test");
    let log_program     = SubProgramId::called("test log");

    scene.add_subprogram(log_program, 
        |_: InputStream<()>, _context| async move {
            async { Result::<(), _>::Err("Oops") }.or_fail("Test").await;
        }, 
        5);

    TestBuilder::new()
        .expect_message_matching(ErrorOutput::Line("\x1b[1;91m!!test log                \x1b[0m | Test: \"Oops\"\n".into()), "Log message did not match")
        .expect_stopped_scene()
        .run_in_scene_with_threads(&scene, test_subprogram, 10);
}

#[test]
fn log_failure_immediately_with_name() {
    let scene = Scene::default();

    let test_subprogram = SubProgramId::called("test");
    let log_program     = SubProgramId::called("test log");

    scene.add_subprogram(log_program, 
        |_: InputStream<()>, context| async move {
            context.i_am("Test program");

            async { Result::<(), _>::Err("Oops") }.or_fail("Test").await;
        }, 
        5);

    TestBuilder::new()
        .expect_message(|msg: ErrorOutput| {
            // The failure can happen either before or after the name is set
            if msg == ErrorOutput::Line("\x1b[1;91m!!Test program            \x1b[0m | Test: \"Oops\"\n".into()) {
                Ok(())
            } else if msg == ErrorOutput::Line("\x1b[1;91m!!test log                \x1b[0m | Test: \"Oops\"\n".into()) {
                Ok(())
            } else {
                Err("Log message did not match".into())
            }
        })
        .expect_stopped_scene()
        .run_in_scene_with_threads(&scene, test_subprogram, 10);
}

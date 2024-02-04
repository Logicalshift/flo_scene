use flo_scene::*;
use flo_scene::programs::*;

use std::time::{Duration};

#[test]
fn basic_timeout() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    TestBuilder::new()
        .send_message(TimerRequest::CallAfter(test_program, 1, Duration::from_millis(10)))
        .expect_message(|_: TimeOut| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn multiple_timeouts() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    TestBuilder::new()
        .send_message(TimerRequest::CallAfter(test_program, 3, Duration::from_millis(15)))
        .send_message(TimerRequest::CallAfter(test_program, 1, Duration::from_millis(5)))
        .send_message(TimerRequest::CallAfter(test_program, 2, Duration::from_millis(10)))
        .expect_message(|TimeOut(id, _)| { if id != 1 { Err(format!("Expected timer 1 first")) } else { Ok(()) } })
        .expect_message(|TimeOut(id, _)| { if id != 2 { Err(format!("Expected timer 2 next")) } else { Ok(()) } })
        .expect_message(|TimeOut(id, _)| { if id != 3 { Err(format!("Expected timer 3 last")) } else { Ok(()) } })
        .run_in_scene(&scene, test_program);
}

/*
#[test]
fn repeating_timeouts() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    TestBuilder::new()
        .send_message(TimerRequest::CallEvery(test_program, 1, Duration::from_millis(5)))
        .expect_message(|_: TimeOut| { Ok(()) })
        .expect_message(|_: TimeOut| { Ok(()) })
        .expect_message(|_: TimeOut| { Ok(()) })
        .run_in_scene(&scene, test_program);
}
*/

use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;

use std::time::{Duration, Instant};

#[test]
fn open_channel() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Try to open the channel to the timner entity and ensure that it's there
            let channel = scene_send_to::<TimerRequest, ()>(TIMER);

            if channel.is_ok() {
                msg.respond(vec![SceneTestResult::Ok]).ok();
            } else {
                msg.respond(vec![SceneTestResult::FailedWithMessage(format!("{:?}", channel.err()))]).ok();
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
fn oneshot() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Ask for timer events to be sent to a new channel
            let mut channel                 = scene_send_to::<TimerRequest, ()>(TIMER).unwrap();
            let (target_channel, receiver)  = SimpleEntityChannel::new(TEST_ENTITY, 5);
            channel.send(TimerRequest::OneShot(TimerId(42), Duration::from_millis(10), target_channel.boxed())).await.unwrap();

            // Should receive a request 10ms later
            let mut receiver                = receiver;
            let Timeout(timer_id, when)     = *receiver.next().await.unwrap();

            if timer_id == TimerId(42) && when == Duration::from_millis(10) {
                msg.respond(vec![SceneTestResult::Ok]).ok();
            } else {
                msg.respond(vec![SceneTestResult::Failed]).ok();
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}


#[test]
fn repeating() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let msg: Message<(), Vec<SceneTestResult>> = msg;

            // Ask for timer events to be sent to a new channel
            let mut channel                 = scene_send_to::<TimerRequest, ()>(TIMER).unwrap();
            let (target_channel, receiver)  = SimpleEntityChannel::new(TEST_ENTITY, 1);
            channel.send(TimerRequest::Repeating(TimerId(42), Duration::from_millis(10), target_channel.boxed())).await.unwrap();
            let start_time                  = Instant::now();

            // Should receive a series of requests at 10ms intervals
            let mut receiver                = receiver;
            let Timeout(timer_id_1, when_1) = *receiver.next().await.unwrap();
            let Timeout(timer_id_2, when_2) = *receiver.next().await.unwrap();
            let Timeout(timer_id_3, when_3) = *receiver.next().await.unwrap();
            let end_time                    = Instant::now().duration_since(start_time);

            receiver.close();

            msg.respond(vec![
                (timer_id_1 == TimerId(42)).into(),
                (timer_id_2 == TimerId(42)).into(),
                (timer_id_3 == TimerId(42)).into(),

                (when_1 == Duration::from_millis(10)).into(),
                (when_2 == Duration::from_millis(20)).into(),
                (when_3 == Duration::from_millis(30)).into(),

                (((end_time.as_millis() as i64) - 30).abs() <= 5).into(),
            ]).ok();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

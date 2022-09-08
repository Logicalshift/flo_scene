use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;

use std::mem;
use std::time::{Duration, Instant};

#[test]
#[cfg(feature="timer")]
fn open_channel() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Try to open the channel to the timer entity and ensure that it's there
            let channel = scene_send_to::<TimerRequest>(TIMER);

            if channel.is_ok() {
                msg.send_without_waiting(SceneTestResult::Ok).await.ok();
            } else {
                msg.send_without_waiting(SceneTestResult::FailedWithMessage(format!("{:?}", channel.err()))).await.ok();
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

#[test]
#[cfg(feature="timer")]
fn oneshot() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Ask for timer events to be sent to a new channel
            let mut channel                 = scene_send_to::<TimerRequest>(TIMER).unwrap();
            let (target_channel, receiver)  = SimpleEntityChannel::new(TEST_ENTITY, 5);
            channel.send_without_waiting(TimerRequest::OneShot(TimerId(42), Duration::from_millis(10), target_channel.boxed())).await.unwrap();

            // Should receive a request 10ms later
            let mut receiver                = receiver;
            let Timeout(timer_id, when)     = receiver.next().await.unwrap();

            if timer_id == TimerId(42) && when == Duration::from_millis(10) {
                msg.send_without_waiting(SceneTestResult::Ok).await.ok();
            } else {
                msg.send_without_waiting(SceneTestResult::Failed).await.ok();
            }
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}


#[test]
#[cfg(feature="timer")]
fn repeating() {
    let scene = Scene::default();

    // Create a test for this scene
    scene.create_entity(TEST_ENTITY, move |_context, mut msg| async move {
        // Whenever a test is requested...
        while let Some(msg) = msg.next().await {
            let SceneTestRequest(mut msg) = msg;

            // Ask for timer events to be sent to a new channel
            let mut channel                 = scene_send_to::<TimerRequest>(TIMER).unwrap();
            let (target_channel, receiver)  = SimpleEntityChannel::new(TEST_ENTITY, 1);
            channel.send_without_waiting(TimerRequest::Repeating(TimerId(42), Duration::from_millis(10), target_channel.boxed())).await.unwrap();
            let start_time                  = Instant::now();

            // Should receive a series of requests at 10ms intervals
            let mut receiver                = receiver;
            let Timeout(timer_id_1, when_1) = receiver.next().await.unwrap();
            let Timeout(timer_id_2, when_2) = receiver.next().await.unwrap();
            let Timeout(timer_id_3, when_3) = receiver.next().await.unwrap();
            let end_time                    = Instant::now().duration_since(start_time);

            mem::drop(receiver);

            msg.send_without_waiting((timer_id_1 == TimerId(42)).into()).await.ok();
            msg.send_without_waiting((timer_id_2 == TimerId(42)).into()).await.ok();
            msg.send_without_waiting((timer_id_3 == TimerId(42)).into()).await.ok();

            msg.send_without_waiting((when_1 == Duration::from_millis(10)).into()).await.ok();
            msg.send_without_waiting((when_2 == Duration::from_millis(20)).into()).await.ok();
            msg.send_without_waiting((when_3 == Duration::from_millis(30)).into()).await.ok();

            msg.send_without_waiting((((end_time.as_millis() as i64) - 30).abs() <= 5).into()).await.ok();
        }
    }).unwrap();

    // Test the scene we just set up
    test_scene(scene);
}

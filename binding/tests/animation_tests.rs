use flo_scene::*;
use flo_scene::programs::*;
use flo_scene_binding::*;

use futures::prelude::*;
use serde::*;

use std::time::{Duration};

#[test]
fn linear_animation() {
    // The animation action is a scene message we use to communicate our result. We use usize here for determinism, the value is t * 1000.0
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct AnimationAction(usize);
    impl SceneMessage for AnimationAction { }

    // The scene has a couple of programs in it (a parent program and a fake timer program we use to generate some expected timeout messages without using a real timer)
    let scene = Scene::default();

    let parent_program_id       = SubProgramId::called("parent_program");
    let fake_timer_program_id   = SubProgramId::called("fake_timer");
    let test_program_id         = SubProgramId::called("test");

    // The parent program runs the animation
    scene.add_subprogram(parent_program_id, |input: InputStream<()>, context| async move {
        println!("Start animation");

        // Run a 5s linear animation (at 60fps but this doesn't matter because we fake up our timeouts). Generate animation actions as a result
        run_animation(&context, AnimationDescription::Linear(5.0), 1.0/60.0, |t| t.into(), BindingAction::new(|t, context| async move {
            println!("Action - t={:?}", t);
            context.send_message(AnimationAction((t * 1000.0) as _)).await.unwrap();
        })).await;

        let mut input = input;
        while let Some(_input) = input.next().await { }
    }, 1);

    // The fake timing program generates some timeout events immediately when it gets a request
    scene.add_subprogram(fake_timer_program_id, |input: InputStream<TimerRequest>, context| async move {
        let mut input = input;
        while let Some(request) = input.next().await {
            println!("Timer: {:?}", request);

            match request {
                TimerRequest::CallEvery(program_id, timer_id, _) => {
                    let mut timeout = context.send(program_id).unwrap();

                    // Send 4 messages (time=0, time=1 frame, time=1 second, time=10 seconds)
                    // Need to wait for idle a few times here as bindings are processed on idle (so a single wait isn't quite enough to trigger the binding action always)
                    context.wait_for_idle(0).await;
                    timeout.send(TimeOut(timer_id, Duration::from_millis(0))).await.unwrap();
                    context.wait_for_idle(0).await;
                    context.wait_for_idle(0).await;
                    context.wait_for_idle(0).await;
                    timeout.send(TimeOut(timer_id, Duration::from_millis(16))).await.unwrap();
                    context.wait_for_idle(0).await;
                    context.wait_for_idle(0).await;
                    context.wait_for_idle(0).await;
                    timeout.send(TimeOut(timer_id, Duration::from_millis(1000))).await.unwrap();
                    context.wait_for_idle(0).await;
                    context.wait_for_idle(0).await;
                    context.wait_for_idle(0).await;
                    timeout.send(TimeOut(timer_id, Duration::from_millis(10000))).await.unwrap();
                    context.wait_for_idle(0).await;
                    context.wait_for_idle(0).await;
                    context.wait_for_idle(0).await;
                }

                _ => { }
            }
        }
    }, 1);

    // Redirect the timer messages to our fake program
    scene.connect_programs((), fake_timer_program_id, StreamId::with_message_type::<TimerRequest>()).unwrap();
    scene.connect_programs((), test_program_id, StreamId::with_message_type::<AnimationAction>()).unwrap();

    TestBuilder::new()
        .expect_message_matching(AnimationAction(0), "t=0 animation action not generated")
        .expect_message_matching(AnimationAction((16.0 / 5000.0 * 1000.0) as _), "t=16ms animation action not generated")
        .expect_message_matching(AnimationAction((1000.0 / 5000.0 * 1000.0) as _), "t=1000ms animation action not generated")
        .expect_message_matching(AnimationAction((5000.0 / 5000.0 * 1000.0) as _), "t=5000ms animation action not generated")
        .run_in_scene(&scene, test_program_id);
}

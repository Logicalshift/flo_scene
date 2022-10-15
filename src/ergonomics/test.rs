use crate::*;

use futures::prelude::*;
use futures::future;
use futures::executor;
use futures_timer::{Delay};

use std::time::{Duration};

///
/// Request to a component: run its tests, and send the results to the specified channel
///
pub struct SceneTestRequest(pub BoxedEntityChannel<'static, SceneTestResult>);

///
/// Result of a test on an entity in a scene
///
#[derive(Debug, PartialEq, Clone)]
pub enum SceneTestResult {
    Failed,
    FailedWithMessage(String),
    Timeout,
    SceneStopped,
    ChannelError(EntityChannelError),
    Panicked(String),
    Ok,
}

impl From<bool> for SceneTestResult {
    fn from(result: bool) -> SceneTestResult {
        if result {
            SceneTestResult::Ok
        } else {
            SceneTestResult::Failed
        }
    }
}

///
/// Runs a test on a scene
///
pub fn test_scene(scene: Scene) {
    // The timeout future is used to abort the test if it takes too long
    let timeout     = Delay::new(Duration::from_secs(10))
        .map(|_| vec![SceneTestResult::Timeout])
        .boxed();

    // The result future causes the test to actually run
    let (results, results_stream)   = SimpleEntityChannel::new(TEST_ENTITY, 1);
    let (results, results_stream)   = PanicEntityChannel::new(results, results_stream, SceneTestResult::Panicked);
    let mut channel                 = scene.send_to(TEST_ENTITY).unwrap();
    let result                      = async move { 
        // Ask the test entity to run the tests
        channel.send(SceneTestRequest(results.boxed()))
            .await
            .unwrap();

        // Collect the results
        results_stream.collect::<Vec<_>>().await
    }.boxed();

    // Run the scene
    let scene       = scene.run()
        .map(|_| vec![SceneTestResult::SceneStopped])
        .boxed();

    // Need to select between the scene, the result and the timeout
    let test_result         = future::select_all(vec![timeout, result, scene]);
    let (test_result, _ ,_) = executor::block_on(test_result);

    assert!(test_result.iter().all(|result| result == &SceneTestResult::Ok), "Scene test failed: {:?}", test_result);
}

///
/// Tests a scene by running a recipe
///
/// Usually the recipe will use the `expect()` function to specify an expected response for the test
///
pub fn test_scene_with_recipe(scene: Scene, recipe: Recipe) {
    // Fetch the context and create a future to run the recipe
    let context = scene.context();
    let result  = async move {
        recipe.run_with_timeout(context, Duration::from_secs(10)).await
    }.boxed_local();

    // Run the scene alongside the recipe
    let scene               = scene.run().map(|_| Err(RecipeError::SceneStopped)).boxed();

    let test_result         = future::select_all(vec![result, scene]);
    let (test_result, _ ,_) = executor::block_on(test_result);

    assert!(test_result.is_ok(), "Test recipe failed: {:?}", test_result.unwrap_err());
}

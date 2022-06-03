use crate::*;

use futures::prelude::*;
use futures::future;
use futures::executor;
use uuid::*;
use futures_timer::{Delay};

use std::time::{Duration};

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
    let mut channel = scene.send_to::<(), Vec<SceneTestResult>>(TEST_ENTITY).unwrap();
    let result      = channel.send(())
        .map(|result| {
            match result {
                Ok(result)  => result,
                Err(err)    => vec![SceneTestResult::ChannelError(err)],
            }
        })
        .boxed();

    // Run the scene
    let scene       = scene.run()
        .map(|_| vec![SceneTestResult::SceneStopped])
        .boxed();

    // Need to select between the scene, the result and the timeout
    let test_result         = future::select_all(vec![timeout, result, scene]);
    let (test_result, _ ,_) = executor::block_on(test_result);

    assert!(test_result.iter().all(|result| result == &SceneTestResult::Ok), "Scene test failed: {:?}", test_result);
}

use crate::*;

use uuid::*;

pub const TEST_ENTITY: EntityId = EntityId::well_known(uuid!["5B93BD5F-39F5-4B57-ABE9-DF593F331E86"]);

///
/// Result of a test on an entity in a scene
///
#[derive(Debug)]
pub enum SceneTestResult {
    Failed,
    FailedWithMessage(String),
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

}

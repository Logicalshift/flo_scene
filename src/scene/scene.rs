use super::scene_core::*;
use crate::entity_id::*;

use ::desync::*;

use std::sync::*;

///
/// A scene encapsulates a set of entities and provides a runtime for them
///
pub struct Scene {
    /// The shared state for all entities in this scene
    core: Arc<Desync<SceneCore>>,
}

impl Default for Scene {
    ///
    /// Creates a scene with the default set of 'well-known' entities
    ///
    fn default() -> Scene {
        Scene::empty()
    }
}

impl Scene {
    ///
    /// Creates a new scene with no entities defined
    ///
    pub fn empty() -> Scene {
        let core    = SceneCore::default();
        let core    = Arc::new(Desync::new(core));

        Scene {
            core
        }
    }

    ///
    /// Runs this scene
    ///
    pub async fn run(self) {
    }
}

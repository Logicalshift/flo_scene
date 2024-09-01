use crate::scene_message::*;

use serde;

///
/// A guest scene message is one that can be sent to a 'guest' scene. These messages are serializable, and are the type
/// that can be sent to or from a guest scene from a host scene.
///
pub trait GuestSceneMessage : SceneMessage + serde::Serialize + for<'de> serde::Deserialize<'de> {

}
use super::stream_id::*;
use crate::scene_message::*;

use serde;

// TODO: would be convenient to automatically generate a type name: maybe doing this via a macro makes sense?
//
// The issue is that std::any::type_name() is not guaranteed to be unique (though it's probably 'unique enough')
// but also is not guaranteed to be stable (so it'll cause incompatibilities down the line). However, we could
// still use it here, with the caveat that the user might need to manually specify the name if they later want
// to use things compiled with different versions of Rust.
//
// With webassembly this is very likely to happen, though, and I'm not sure the initial convenience is worth the
// later confusion.
//

///
/// A guest scene message is one that can be sent to a 'guest' scene. These messages are serializable, and are the type
/// that can be sent to or from a guest scene from a host scene.
///
pub trait GuestSceneMessage : SceneMessage + serde::Serialize + for<'de> serde::Deserialize<'de> {
    /// Returns the stream ID for this message (this is a unique identifier for this type)
    fn stream_id() -> HostStreamId;
}

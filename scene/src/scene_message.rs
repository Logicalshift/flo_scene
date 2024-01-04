///
/// Trait implemented by messages that can be sent via a scene
///
pub trait SceneMessage : Sized + Send + Sync + Unpin {

}

impl SceneMessage for () { }
impl SceneMessage for String { }
impl SceneMessage for char { }
impl SceneMessage for usize { }
impl SceneMessage for isize { }
impl SceneMessage for i8 { }
impl SceneMessage for u8 { }
impl SceneMessage for i16 { }
impl SceneMessage for u16 { }
impl SceneMessage for i32 { }
impl SceneMessage for u32 { }
impl SceneMessage for i64 { }
impl SceneMessage for u64 { }
impl SceneMessage for i128 { }
impl SceneMessage for u128 { }

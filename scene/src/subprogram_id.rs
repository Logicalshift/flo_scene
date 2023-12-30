use uuid::*;

// TODO: store the internal ID referenced via a handle (so SubProgramId can implement Copy + be small/fast to reference)

///
/// A unique identifier for a subprogram in a scene
///
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub enum SubProgramId {
    /// A subprogram identified with a well-known name
    Named(String),

    /// A subprogram identified with a GUID
    Guid(Uuid),
}

impl SubProgramId {
    ///
    /// Creates a new unique subprogram id
    ///
    pub fn new() -> SubProgramId {
        SubProgramId::Guid(Uuid::new_v4())
    }

    ///
    /// Creates a subprogram ID with a well-known name
    ///
    pub fn called(name: &str) -> SubProgramId {
        SubProgramId::Named(name.into())
    }
}

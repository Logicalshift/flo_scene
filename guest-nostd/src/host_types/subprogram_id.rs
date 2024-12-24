use ::serde::*;
use ::serde::de::*;
use uuid::*;

///
/// A unique identifier for a subprogram in a scene
///
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(Debug)]    // TODO!
pub struct SubProgramId(SubProgramIdValue);

///
/// A subprogram name ID
///
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
#[derive(Serialize, Deserialize)]       // TODO!
struct SubProgramNameId(usize);

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
#[derive(Serialize, Deserialize)]
enum SubProgramIdValue {
    /// A subprogram identified with a well-known name
    Named(SubProgramNameId),

    /// A subprogram identified with a GUID
    Guid(Uuid),

    /// A task created by a named subprogram. The second 'usize' value is a unique serial number for this task
    ///
    /// Tasks differ from subprograms in that they have a limited lifespan and read an input stream specified at creation
    NamedTask(SubProgramNameId, usize),

    /// A task created by a GUID subprogram. The 'usize' value is a unique serial number for this task
    ///
    /// Tasks differ from subprograms in that they have a limited lifespan and read an input stream specified at creation
    GuidTask(Uuid, usize),
}

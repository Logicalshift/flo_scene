use crate::{SubProgramId};

///
/// Describes the source of a stream
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum StreamSource {
    /// All sources of this type of stream
    All,

    /// Take the stream from a particular program
    Program(SubProgramId),
}

impl From<SubProgramId> for StreamSource {
    #[inline]
    fn from(program: SubProgramId) -> StreamSource {
        StreamSource::Program(program)
    }
}

impl From<()> for StreamSource {
    #[inline]
    fn from(_: ()) -> StreamSource {
        StreamSource::All
    }
}

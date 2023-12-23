use crate::{SubProgramId};

///
/// Describes the source of a stream
///
#[derive(Clone, Copy, PartialEq, Debug)]
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

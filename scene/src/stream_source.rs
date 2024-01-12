use crate::{SubProgramId};

///
/// Describes the source of a stream
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum StreamSource {
    /// All sources of this type of stream
    All,

    /// A stream of this type originating from a specific program
    Program(SubProgramId),

    /// A stream of this type originating from any program but with a specific program given as the target
    TargetProgram(SubProgramId),
}

impl StreamSource {
    ///
    /// Returns true if this stream source matches a particular subprogram
    ///
    pub fn matches_subprogram(&self, id: &SubProgramId) -> bool {
        match self {
            StreamSource::All                       => true,
            StreamSource::Program(source_id)        => source_id.eq(id),
            StreamSource::TargetProgram(_target_id) => true,
        }
    }
}

impl From<SubProgramId> for StreamSource {
    #[inline]
    fn from(program: SubProgramId) -> StreamSource {
        StreamSource::Program(program)
    }
}

impl<'a> From<&'a SubProgramId> for StreamSource {
    #[inline]
    fn from(program: &'a SubProgramId) -> StreamSource {
        StreamSource::Program(program.clone())
    }
}

impl From<()> for StreamSource {
    #[inline]
    fn from(_: ()) -> StreamSource {
        StreamSource::All
    }
}

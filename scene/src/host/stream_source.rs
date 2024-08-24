use crate::filter::*;
use crate::subprogram_id::*;

#[cfg(feature="serde_support")] use serde::*;

///
/// Describes the source of a stream
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub enum StreamSource {
    /// All sources of this type of stream
    All,

    /// Any source that can be transformed to the input of its target via the specified filter
    Filtered(FilterHandle),

    /// A stream of this type originating from a specific program
    Program(SubProgramId),
}

impl StreamSource {
    ///
    /// Returns true if this stream source matches a particular subprogram
    ///
    pub fn matches_subprogram(&self, id: &SubProgramId) -> bool {
        match self {
            StreamSource::All                       => true,
            StreamSource::Filtered(_)               => true,
            StreamSource::Program(source_id)        => source_id.eq(id),
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
        StreamSource::Program(*program)
    }
}

impl From<FilterHandle> for StreamSource {
    #[inline]
    fn from(filter: FilterHandle) -> StreamSource {
        StreamSource::Filtered(filter)
    }
}

impl<'a> From<&'a FilterHandle> for StreamSource {
    #[inline]
    fn from(filter: &'a FilterHandle) -> StreamSource {
        StreamSource::Filtered(*filter)
    }
}

impl From<()> for StreamSource {
    #[inline]
    fn from(_: ()) -> StreamSource {
        StreamSource::All
    }
}

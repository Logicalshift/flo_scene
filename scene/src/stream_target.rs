use crate::filter::*;
use crate::subprogram_id::*;

///
/// A stream target describes where the output of a particular stream should be sent
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum StreamTarget {
    /// Discard any output sent to this stream
    None,

    /// Send output for this stream to the default target for the scene (or defer until a default target is set)
    Any,

    /// Send the stream to the input of the specified program
    Program(SubProgramId),

    /// Send the stream to a subprogram after running through a filter
    Filtered(FilterHandle, SubProgramId),
}

impl StreamTarget {
    ///
    /// The program ID that this target will connect to
    ///
    pub fn target_sub_program(&self) -> Option<SubProgramId> {
        match self {
            StreamTarget::None | StreamTarget::Any                              => None,
            StreamTarget::Program(prog_id) | StreamTarget::Filtered(_, prog_id) => Some(*prog_id),
        }
    }
}

impl From<SubProgramId> for StreamTarget {
    #[inline]
    fn from(program: SubProgramId) -> StreamTarget {
        StreamTarget::Program(program)
    }
}

impl<'a> From<&'a SubProgramId> for StreamTarget {
    #[inline]
    fn from(program: &'a SubProgramId) -> StreamTarget {
        StreamTarget::Program(*program)
    }
}

/// '()' can be used in place of StreamTarget::Any
impl From<()> for StreamTarget {
    #[inline]
    fn from(_: ()) -> StreamTarget {
        StreamTarget::Any
    }
}
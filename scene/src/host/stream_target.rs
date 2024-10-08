use crate::host::filter::*;
use crate::host::subprogram_id::*;

use serde::*;

///
/// A stream target describes where the output of a particular stream should be sent
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
pub enum StreamTarget {
    /// Discard any output sent to this stream
    None,

    /// Send output for this stream to the default target for the scene (or defer until a default target is set)
    Any,

    /// Send the stream to the input of the specified program
    Program(SubProgramId),

    /// Send the stream to a subprogram after running through a filter
    ///
    /// Note that this cannot be combined with `StreamSource::Filtered()`. 
    ///
    /// When connecting this is the equivalent of using `StreamSource::Filtered(filter_handle)` with 
    /// `StreamTarget::Program(program_id)`.
    ///
    /// This can be combined with `StreamSource::Program()` can filter only the output of a single
    /// program. It is also useful as a target for the `StreamContext::send()` call for deliberately
    /// filtering the output of an existing program.
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
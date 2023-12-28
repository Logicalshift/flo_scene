use crate::{SubProgramId};

///
/// A stream target describes where the output of a particular stream should be sent
///
#[derive(Clone, PartialEq, Hash, Debug)]
pub enum StreamTarget {
    /// Discard any output sent to this stream
    None,

    /// Send output for this stream to the default target for the scene (or defer until a default target is set)
    Any,

    /// Send the stream to the input of the specified program
    Program(SubProgramId),
}

impl From<SubProgramId> for StreamTarget {
    #[inline]
    fn from(program: SubProgramId) -> StreamTarget {
        StreamTarget::Program(program)
    }
}

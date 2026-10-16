use crate::host::stream_id::*;
use crate::host::subprogram_id::*;

use std::vec::{IntoIter};

///
/// The set of streams affected by a change to the connectome
///
/// Each entry is a (source program, stream) pair whose cached route was invalidated. Callers should
/// reconnect these streams so that they pick up their new route.
///
#[derive(Clone, Debug, Default)]
pub struct InvalidatedStreams {
    streams: Vec<(SubProgramId, StreamId)>,
}

impl InvalidatedStreams {
    ///
    /// The streams that are affected by the change
    ///
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item=&(SubProgramId, StreamId)> {
        self.streams.iter()
    }

    ///
    /// True if no streams were affected by the change
    ///
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }

    ///
    /// The number of streams that were affected by the change
    ///
    #[inline]
    pub fn len(&self) -> usize {
        self.streams.len()
    }

    ///
    /// Adds a stream to this list, ignoring duplicates
    ///
    pub (crate) fn push(&mut self, source_program: SubProgramId, stream: StreamId) {
        let affected_stream = (source_program, stream);

        if !self.streams.contains(&affected_stream) {
            self.streams.push(affected_stream);
        }
    }
}

impl IntoIterator for InvalidatedStreams {
    type Item       = (SubProgramId, StreamId);
    type IntoIter   = IntoIter<(SubProgramId, StreamId)>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.streams.into_iter()
    }
}

impl<'a> IntoIterator for &'a InvalidatedStreams {
    type Item = &'a (SubProgramId, StreamId);
    type IntoIter = std::slice::Iter<'a, (SubProgramId, StreamId)>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.streams.iter()
    }
}

impl FromIterator<(SubProgramId, StreamId)> for InvalidatedStreams {
    fn from_iter<TIter: IntoIterator<Item = (SubProgramId, StreamId)>>(iter: TIter) -> Self {
        let mut streams = InvalidatedStreams::default();

        for stream in iter {
            streams.streams.push(stream);
        }

        streams
    }
}

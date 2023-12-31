use crate::input_stream::*;

use futures::prelude::*;

///
/// A filter is a way to convert from a stream of one message type to another, and a filter
/// handle references a predefined filter.
///
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilterHandle(usize);

impl FilterHandle {
    ///
    /// Returns a filter handle for a filtering function
    ///
    /// A filter can be used to convert between an output of one subprogram and the input of another when they are different types. This makes it
    /// possible to connect subprograms without needing an intermediate program that performs the conversion.
    ///
    pub fn for_filter<TSourceMessage, TTargetStream>(filter: impl 'static + Send + Sync + Fn(InputStream<TSourceMessage>) -> TTargetStream) -> FilterHandle
    where
        TSourceMessage:         'static + Unpin + Send + Sync,
        TTargetStream:          Stream,
        TTargetStream::Item:    'static + Unpin + Send + Sync,
    {
        todo!()
    }
}
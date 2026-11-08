use super::filter::*;

///
/// Stream that re-applies program IDs to the other side of a filter stream
///
pub struct FilterChainStream<TSourceStream, TTargetMessage> {
    /// Chained filter, which generates a stream of TTargetMessage
    filter: FilterHandle,

    /// Source for the filter
    source: TSourceStream,
}


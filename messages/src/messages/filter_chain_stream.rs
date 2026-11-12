use crate::errors::*;
use super::filter::*;
use super::subprogram_id::*;
use super::traits::*;

use futures::prelude::*;
use futures::stream::{BoxStream};
use futures::task::{Context, Poll};

use alloc::sync::*;
use core::any::{TypeId};
use core::pin::*;

///
/// Stream that re-applies program IDs to the other side of a filter stream
///
/// Filters produce plain streams of messages, without any information about which program sent them. This stream runs a filter
/// against a source `SceneStream` and generates a `SceneStream` from the result, with each output message tagged with the program
/// ID of the input messages that produced it.
///
/// Filters are usually used with a single source program, so the filter is started for the first program ID that is seen and runs
/// until a message from a different program arrives. At that point the filter's input is closed and the remaining output is read,
/// then a new instance of the filter is started for the new program ID. If a filter finishes before its input is closed, the
/// remaining messages from that program are discarded until a message from a different program arrives.
///
pub struct FilterChainStream<TSourceMessage, TTargetMessage> {
    /// Chained filter, which generates a stream of TTargetMessage
    filter: FilterHandle,

    /// Source for the filter
    source: Arc<Mutex<FilterChainSource<TSourceMessage>>>,

    /// The program ID and the output of the currently running filter
    current: Option<(SubProgramId, BoxStream<'static, TTargetMessage>)>,

    /// After a filter has finished, messages from this program are discarded until a message from another program arrives
    draining: Option<SubProgramId>,
}

///
/// The source stream, shared between the chain stream and the input of the current filter
///
struct FilterChainSource<TSourceMessage> {
    /// The source messages, or None if the source stream has finished
    stream: Option<BoxStream<'static, (SubProgramId, TSourceMessage)>>,

    /// A message that has been read from the source but not yet sent to a filter
    pending: Option<(SubProgramId, TSourceMessage)>,
}

///
/// Input stream for a single filter: reads messages from the source until the program ID changes
///
struct FilterSegmentStream<TSourceMessage> {
    /// The program that this segment is reading messages from
    program_id: SubProgramId,

    /// The source stream
    source: Arc<Mutex<FilterChainSource<TSourceMessage>>>,

    /// Set to true once the program ID changes or the source finishes
    finished: bool,
}

impl<TSourceMessage> Stream for FilterSegmentStream<TSourceMessage> {
    type Item = TSourceMessage;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished { return Poll::Ready(None); }

        let program_id  = self.program_id;
        let source      = Arc::clone(&self.source);
        let mut source  = source.lock().unwrap();

        // Read a message (either the one waiting in the source, or the next message from the stream)
        let next = if let Some(pending) = source.pending.take() {
            pending
        } else if let Some(stream) = source.stream.as_mut() {
            match stream.poll_next_unpin(cx) {
                Poll::Pending           => { return Poll::Pending; }
                Poll::Ready(Some(next)) => next,
                Poll::Ready(None)       => {
                    source.stream   = None;
                    self.finished   = true;
                    return Poll::Ready(None);
                }
            }
        } else {
            self.finished = true;
            return Poll::Ready(None);
        };

        if next.0 == program_id {
            Poll::Ready(Some(next.1))
        } else {
            // The program ID has changed: leave the message for the next filter and close this input stream
            source.pending  = Some(next);
            self.finished   = true;
            Poll::Ready(None)
        }
    }
}

impl<TSourceMessage> SceneStream for FilterSegmentStream<TSourceMessage>
where
    TSourceMessage: SceneMessage,
{
    fn messages_with_sources_boxed(self: Pin<Box<Self>>) -> BoxStream<'static, (SubProgramId, Self::Item)> {
        let program_id = self.program_id;
        self.map(move |msg| (program_id, msg)).boxed()
    }
}

impl<TSourceMessage, TTargetMessage> FilterChainStream<TSourceMessage, TTargetMessage>
where
    TSourceMessage: SceneMessage,
    TTargetMessage: SceneMessage,
{
    ///
    /// Creates a new filter chain stream that will run the specified filter against the messages in a source stream
    ///
    pub fn new(filter: FilterHandle, source: impl 'static + SceneStream<Item=TSourceMessage>) -> Result<Self, ConnectionError> {
        // Check that the filter accepts and generates the message types that this stream uses
        if filter.source_stream_id().message_type() != TypeId::of::<TSourceMessage>() { return Err(ConnectionError::FilterInputDoesNotMatch); }
        if filter.target_stream_id().message_type() != TypeId::of::<TTargetMessage>() { return Err(ConnectionError::FilterOutputDoesNotMatch); }

        let source = FilterChainSource {
            stream:     Some(source.messages_with_sources()),
            pending:    None,
        };

        Ok(Self {
            filter:     filter,
            source:     Arc::new(Mutex::new(source)),
            current:    None,
            draining:   None,
        })
    }

    ///
    /// Polls for the next message, along with the program ID of the messages that the filter was reading when it was generated
    ///
    fn poll_next_with_source(&mut self, cx: &mut Context<'_>) -> Poll<Option<(SubProgramId, TTargetMessage)>> {
        loop {
            // Read from the current filter if there is one
            if let Some((program_id, filter_output)) = self.current.as_mut() {
                match filter_output.poll_next_unpin(cx) {
                    Poll::Ready(Some(msg))  => { return Poll::Ready(Some((*program_id, msg))); }
                    Poll::Pending           => { return Poll::Pending; }
                    Poll::Ready(None)       => {
                        // Filter has finished: discard any further messages for this program (if its input was closed because the program ID changed, this does nothing)
                        self.draining   = Some(*program_id);
                        self.current    = None;
                    }
                }
            }

            // Wait for a message with a new program ID to arrive
            let next_program_id = {
                let mut source = self.source.lock().unwrap();

                loop {
                    let next = if let Some(pending) = source.pending.take() {
                        pending
                    } else if let Some(stream) = source.stream.as_mut() {
                        match stream.poll_next_unpin(cx) {
                            Poll::Pending           => { return Poll::Pending; }
                            Poll::Ready(Some(next)) => next,
                            Poll::Ready(None)       => {
                                source.stream = None;
                                return Poll::Ready(None);
                            }
                        }
                    } else {
                        return Poll::Ready(None);
                    };

                    if Some(next.0) == self.draining {
                        // Discard messages for a program whose filter has already finished
                        continue;
                    }

                    // Found the start of a new run of messages: leave the message for the filter to read
                    let program_id  = next.0;
                    source.pending  = Some(next);
                    self.draining   = None;

                    break program_id;
                }
            };

            // Start a new filter for this program
            let segment = FilterSegmentStream {
                program_id: next_program_id,
                source:     Arc::clone(&self.source),
                finished:   false,
            };

            match self.filter.filter::<TSourceMessage, TTargetMessage>(segment) {
                Ok(filter_output)   => { self.current = Some((next_program_id, filter_output)); }
                Err(_)              => {
                    // The types are checked when the stream is created, so this shouldn't happen
                    debug_assert!(false, "Filter types do not match");
                    return Poll::Ready(None);
                }
            }
        }
    }
}

impl<TSourceMessage, TTargetMessage> Stream for FilterChainStream<TSourceMessage, TTargetMessage>
where
    TSourceMessage: SceneMessage,
    TTargetMessage: SceneMessage,
{
    type Item = TTargetMessage;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_with_source(cx).map(|msg| msg.map(|(_, msg)| msg))
    }
}

impl<TSourceMessage, TTargetMessage> SceneStream for FilterChainStream<TSourceMessage, TTargetMessage>
where
    TSourceMessage: SceneMessage,
    TTargetMessage: SceneMessage,
{
    fn messages_with_sources_boxed(mut self: Pin<Box<Self>>) -> BoxStream<'static, (SubProgramId, Self::Item)> {
        stream::poll_fn(move |cx| self.poll_next_with_source(cx)).boxed()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::executor;

    struct TestSceneStream<TMessage> {
        messages: BoxStream<'static, (SubProgramId, TMessage)>,
    }

    impl<TMessage> Stream for TestSceneStream<TMessage> {
        type Item = TMessage;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            self.messages.poll_next_unpin(cx).map(|msg| msg.map(|(_, msg)| msg))
        }
    }

    impl<TMessage: SceneMessage> SceneStream for TestSceneStream<TMessage> {
        fn messages_with_sources_boxed(self: Pin<Box<Self>>) -> BoxStream<'static, (SubProgramId, Self::Item)> {
            Pin::into_inner(self).messages
        }
    }

    fn source(messages: Vec<(SubProgramId, usize)>) -> TestSceneStream<usize> {
        TestSceneStream { messages: stream::iter(messages).boxed() }
    }

    #[test]
    fn single_program() {
        let program     = SubProgramId::new();
        let to_string   = FilterHandle::for_filter(|input: BoxSceneStream<usize>| input.map(|num| num.to_string()));
        let chain       = FilterChainStream::<usize, String>::new(to_string, source(vec![(program, 1), (program, 2), (program, 3)])).unwrap();

        let output = executor::block_on(chain.messages_with_sources().collect::<Vec<_>>());

        assert_eq!(output, vec![(program, "1".to_string()), (program, "2".to_string()), (program, "3".to_string())]);
    }

    #[test]
    fn restart_on_program_change() {
        let program_a   = SubProgramId::new();
        let program_b   = SubProgramId::new();

        // Filter that sums all of its input, so we can see when it's restarted
        let sum         = FilterHandle::for_filter(|input: BoxSceneStream<usize>| input.fold(0usize, |a, b| async move { a + b }).into_stream().map(|sum| sum.to_string()));
        let chain       = FilterChainStream::<usize, String>::new(sum, source(vec![(program_a, 1), (program_a, 2), (program_b, 3), (program_b, 4), (program_a, 5)])).unwrap();

        let output = executor::block_on(chain.messages_with_sources().collect::<Vec<_>>());

        assert_eq!(output, vec![(program_a, "3".to_string()), (program_b, "7".to_string()), (program_a, "5".to_string())]);
    }

    #[test]
    fn discard_after_filter_finishes() {
        let program_a   = SubProgramId::new();
        let program_b   = SubProgramId::new();

        let first_only  = FilterHandle::for_filter(|input: BoxSceneStream<usize>| input.take(1).map(|num| num.to_string()));
        let chain       = FilterChainStream::<usize, String>::new(first_only, source(vec![(program_a, 1), (program_a, 2), (program_b, 3), (program_b, 4), (program_a, 5)])).unwrap();

        let output = executor::block_on(chain.messages_with_sources().collect::<Vec<_>>());

        assert_eq!(output, vec![(program_a, "1".to_string()), (program_b, "3".to_string()), (program_a, "5".to_string())]);
    }

    #[test]
    fn filter_that_reads_nothing() {
        let program_a   = SubProgramId::new();
        let program_b   = SubProgramId::new();

        let empty       = FilterHandle::for_filter(|_input: BoxSceneStream<usize>| stream::empty::<String>());
        let chain       = FilterChainStream::<usize, String>::new(empty, source(vec![(program_a, 1), (program_b, 2)])).unwrap();

        let output = executor::block_on(chain.messages_with_sources().collect::<Vec<_>>());

        assert_eq!(output, vec![]);
    }

    #[test]
    fn reject_mismatched_types() {
        let to_string = FilterHandle::for_filter(|input: BoxSceneStream<usize>| input.map(|num| num.to_string()));

        assert!(matches!(FilterChainStream::<usize, usize>::new(to_string, source(vec![])), Err(ConnectionError::FilterOutputDoesNotMatch)));
    }
}

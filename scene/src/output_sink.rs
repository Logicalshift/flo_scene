use crate::{input_stream::*, SubProgramId};

use futures::prelude::*;
use futures::task::{Poll, Waker};

use std::pin::*;
use std::sync::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_IDENTIFIER: AtomicUsize = AtomicUsize::new(0);

///
/// The target of an output sink
///
pub (crate) enum OutputSinkTarget<TMessage> {
    /// Indicates an output that has nowhere to send its data (will just block)
    Disconnected,

    /// Indicates an output that discards its data
    Discard,

    /// Indicates an output that sends its data to another subprogram's input
    Input(Weak<Mutex<InputStreamCore<TMessage>>>),
}

///
/// An output sink is a way for a subprogram to send messages to the input of another subprogram
///
pub struct OutputSink<TMessage> {
    /// A unique identifier for this output sink
    identifier: usize,

    /// The ID of the program that owns this output
    program_id: SubProgramId,

    /// Where the data for this sink should be sent
    target: OutputSinkTarget<TMessage>,

    /// The message that is being sent
    waiting_message: Option<TMessage>,

    /// Waker that is notified when the target is changed
    when_target_changed: Option<Waker>,
}

impl<TMessage> OutputSink<TMessage> {
    ///
    /// Creates a new output sink that belongs to the specified sub-program
    ///
    pub (crate) fn new(program_id: SubProgramId) -> OutputSink<TMessage> {
        OutputSink {
            identifier:             NEXT_IDENTIFIER.fetch_add(1, Ordering::Relaxed),
            program_id:             program_id,
            target:                 OutputSinkTarget::Disconnected,
            waiting_message:        None,
            when_target_changed:    None,
        }
    }

    ///
    /// Sends the messages from this sink to a particular input stream
    ///
    pub (crate) fn attach_to(&mut self, input_stream: &InputStream<TMessage>) {
        self.attach_to_core(&input_stream.core);
    }

    ///
    /// Sends the messages from this sink to an input stream core
    ///
    pub (crate) fn attach_to_core(&mut self, input_stream_core: &Arc<Mutex<InputStreamCore<TMessage>>>) {
        // Connect to the target
        self.target = OutputSinkTarget::Input(Arc::downgrade(input_stream_core));

        // Wake anything waiting for the stream to become ready or to send a message
        if let Some(waker) = self.when_target_changed.take() {
            waker.wake();
        }
    }
}

impl<TMessage> Sink<TMessage> for OutputSink<TMessage> 
where
    TMessage: Unpin,
{
    type Error = ();

    fn poll_ready(mut self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Always say that we're ready (we store the message in the sink while we're flushing instead)
        match &self.target {
            OutputSinkTarget::Disconnected  => {
                self.when_target_changed = Some(context.waker().clone());
                Poll::Pending
            },
            OutputSinkTarget::Discard       => Poll::Ready(Ok(())),
            OutputSinkTarget::Input(_)      => Poll::Ready(Ok(())),
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: TMessage) -> Result<(), Self::Error> {
        match &self.target {
            OutputSinkTarget::Disconnected  => Ok(()),
            OutputSinkTarget::Discard       => Ok(()),
            OutputSinkTarget::Input(core)   => {
                if let Some(core) = core.upgrade() {
                    // Either directly send the item or add to the callback list for when there's enough space in the input
                    let mut core = core.lock().unwrap();

                    match core.send(item) {
                        Ok(()) => {
                            self.waiting_message = None;
                            Ok(())
                        }

                        Err(item) => {
                            // Need to wait for a slot in the stream
                            self.waiting_message = Some(item);
                            Ok(())
                        }
                    }
                } else {
                    // We'll sleep until a new core is connected
                    Ok(())
                }
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.when_target_changed = None;

        match &self.target {
            OutputSinkTarget::Disconnected  => {
                // Wait for the target to change
                self.when_target_changed = Some(context.waker().clone());
                Poll::Pending
            },

            OutputSinkTarget::Discard       => Poll::Ready(Ok(())),

            OutputSinkTarget::Input(core)   => {
                if let Some(core) = core.upgrade() {
                    if let Some(message) = self.waiting_message.take() {
                        // Try sending the waiting message
                        let mut core = core.lock().unwrap();

                        match core.send(message) {
                            Ok(()) => {
                                self.waiting_message = None;
                                Poll::Ready(Ok(()))
                            }

                            Err(message) => {
                                // Need to wait for a slot in the stream
                                self.waiting_message        = Some(message);
                                self.when_target_changed    = Some(context.waker().clone());
                                core.wake_when_slots_available(context);
                                Poll::Pending
                            }
                        }
                    } else {
                        // No message is waiting
                        Poll::Ready(Ok(()))
                    }
                } else {
                    // Core has been released, so we wait as if disconnected
                    self.when_target_changed = Some(context.waker().clone());
                    Poll::Pending
                }
            }
        }
    }

    fn poll_close(self: Pin<&mut Self>, _context: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Output is always flushed straight to the input stream, and the input stream is closed when the program finishes
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use futures::future::{poll_fn};
    use futures::executor;
    use futures::pin_mut;

    #[test]
    fn send_message_to_input_stream() {
        // Create an input stream and an output sink
        let program_id          = SubProgramId::new();
        let mut input_stream    = InputStream::<u32>::new(program_id.clone(), 1000);
        let mut output_sink     = OutputSink::new(program_id.clone());

        // Attach the output sink to the input stream
        output_sink.attach_to(&input_stream);

        executor::block_on(async move {
            // Send some messages to the stream from the sink
            output_sink.send(1).await.unwrap();
            output_sink.send(2).await.unwrap();

            // Stream should retrieve those messages
            assert!(input_stream.next().await == Some(1));
            assert!(input_stream.next().await == Some(2));
        })
    }

    #[test]
    fn send_message_to_input_stream_from_multiple_sinks() {
        // Create an input stream and an output sink
        let program_id          = SubProgramId::new();
        let mut input_stream    = InputStream::<u32>::new(program_id.clone(), 1000);
        let mut output_sink_1   = OutputSink::new(program_id.clone());
        let mut output_sink_2   = OutputSink::new(program_id.clone());

        // Attach the output sink to the input stream
        output_sink_1.attach_to(&input_stream);
        output_sink_2.attach_to(&input_stream);

        executor::block_on(async move {
            // Send some messages to the stream from both sinks (we shouldn't block here because )
            output_sink_1.send(1).await.unwrap();
            output_sink_2.send(2).await.unwrap();

            // Stream should retrieve those messages
            assert!(input_stream.next().await == Some(1));
            assert!(input_stream.next().await == Some(2));
        })
    }

    #[test]
    fn send_message_to_full_input_stream() {
        // Create an input stream and an output sink
        let program_id          = SubProgramId::new();
        let mut input_stream    = InputStream::<u32>::new(program_id.clone(), 0);
        let mut output_sink     = OutputSink::new(program_id.clone());

        // Attach the output sink to the input stream
        output_sink.attach_to(&input_stream);

        executor::block_on(async move {
            // First message will send OK
            output_sink.send(1).await.unwrap();

            // Second message will be blocked by the first
            let blocked_send = output_sink.send(2);
            pin_mut!(blocked_send);
            assert!((&mut blocked_send).now_or_never().is_none());

            // Stream should retrieve those messages
            assert!(input_stream.next().await == Some(1));

            // Should now send the next value to the sink
            assert!((&mut blocked_send).now_or_never().is_some());
            assert!(input_stream.next().await == Some(2));
        })
    }

    #[test]
    fn send_message_to_disconnected_input_stream() {
        // Create an input stream and an output sink
        let program_id          = SubProgramId::new();
        let mut input_stream    = InputStream::<u32>::new(program_id.clone(), 0);
        let mut output_sink     = OutputSink::new(program_id.clone());

        executor::block_on(async move {
            // Sending a message will block while the output sink is disconnected
            let _ = poll_fn(|ctxt| Poll::Ready(output_sink.poll_ready_unpin(ctxt))).await;
            output_sink.start_send_unpin(2).unwrap();
            assert!(poll_fn(|ctxt| Poll::Ready(output_sink.poll_flush_unpin(ctxt))).await == Poll::Pending);

            // Attach the input stream to the output
            output_sink.attach_to(&input_stream);

            // Should now send the blocked value to the sink
            assert!(poll_fn(|ctxt| Poll::Ready(output_sink.poll_flush_unpin(ctxt))).await == Poll::Ready(Ok(())));
            assert!(input_stream.next().await == Some(2));
        })
    }
}
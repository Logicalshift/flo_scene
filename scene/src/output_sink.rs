use crate::{input_stream::*, SubProgramId};

use futures::prelude::*;
use futures::task::{Poll, Waker};

use std::pin::*;
use std::sync::*;

// TODO: close the sink when the target program finishes

///
/// The target of an output sink
///
#[derive(Clone)]
pub (crate) enum OutputSinkTarget<TMessage> {
    /// Indicates an output that has nowhere to send its data (will just block)
    Disconnected,

    /// Indicates an output that discards its data
    Discard,

    /// Indicates an output that sends its data to another subprogram's input
    Input(Weak<Mutex<InputStreamCore<TMessage>>>),

    /// Same as 'Input', except the stream is closed when this output sink target is dropped
    CloseWhenDropped(Weak<Mutex<InputStreamCore<TMessage>>>),
}

///
/// The shared core of an output sink
///
pub (crate) struct OutputSinkCore<TMessage> {
    /// The target for the sink
    pub (crate) target: OutputSinkTarget<TMessage>,

    /// Waker that is notified when the target is changed
    pub (crate) when_target_changed: Option<Waker>,
}

///
/// An output sink is a way for a subprogram to send messages to the input of another subprogram
///
pub struct OutputSink<TMessage> {
    /// The ID of the program that owns this output
    program_id: SubProgramId,

    /// Where the data for this sink should be sent
    core: Arc<Mutex<OutputSinkCore<TMessage>>>,

    /// The message that is being sent
    waiting_message: Option<TMessage>,

    /// Waker that is notified when a pending message is sent
    when_message_sent: Option<Waker>,
}

impl<TMessage> Drop for OutputSinkTarget<TMessage> {
    fn drop(&mut self) {
        match self {
            OutputSinkTarget::CloseWhenDropped(core) => {
                if let Some(core) = core.upgrade() {
                    let waker = core.lock().unwrap().close();

                    if let Some(waker) = waker {
                        waker.wake();
                    }
                }
            }

            _ => { }
        }
    }
}

impl<TMessage> OutputSinkCore<TMessage> {
    ///
    /// Creates a new output sink core
    ///
    pub (crate) fn new(target: OutputSinkTarget<TMessage>) -> Self {
        OutputSinkCore {
            target:                 target,
            when_target_changed:    None,
        }
    }
}

impl<TMessage> OutputSink<TMessage> {
    ///
    /// Creates a new output sink that is attached to a known target
    ///
    pub (crate) fn attach(program_id: SubProgramId, core: Arc<Mutex<OutputSinkCore<TMessage>>>) -> OutputSink<TMessage> {
        OutputSink {
            program_id:             program_id,
            core:                   core,
            waiting_message:        None,
            when_message_sent:      None,
        }
    }
}

impl<TMessage> Sink<TMessage> for OutputSink<TMessage> 
where
    TMessage: Unpin,
{
    type Error = ();

    fn poll_ready(mut self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Say we're waiting if there's an input value waiting
        if self.waiting_message.is_some() {
            // Wait for the message to finish sending
            self.when_message_sent = Some(context.waker().clone());
            Poll::Pending
        } else {
            // Always say that we're ready (we store the message in the sink while we're flushing instead)
            let mut core = self.core.lock().unwrap();

            match &core.target {
                OutputSinkTarget::Disconnected  => {
                    core.when_target_changed = Some(context.waker().clone());
                    Poll::Pending
                },
                OutputSinkTarget::Discard               => Poll::Ready(Ok(())),
                OutputSinkTarget::Input(_)              => Poll::Ready(Ok(())),
                OutputSinkTarget::CloseWhenDropped(_)   => Poll::Ready(Ok(())),
            }
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: TMessage) -> Result<(), Self::Error> {
        use std::mem;

        let core = self.core.lock().unwrap();
        match &core.target {
            OutputSinkTarget::Disconnected                  => {
                mem::drop(core);
                self.waiting_message = Some(item);
                Ok(())
            },

            OutputSinkTarget::Discard                       => Ok(()),

            OutputSinkTarget::Input(input_core)             |
            OutputSinkTarget::CloseWhenDropped(input_core)  => {
                if let Some(input_core) = input_core.upgrade() {
                    // Either directly send the item or add to the callback list for when there's enough space in the input
                    mem::drop(core);
                    let mut input_core = input_core.lock().unwrap();

                    match input_core.send(self.program_id, item) {
                        Ok(waker) => {
                            // Sent the message: wake up anything waiting for the input stream
                            self.waiting_message = None;
                            mem::drop(input_core);

                            if let Some(waker) = waker { waker.wake() };
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
        use std::mem;

        // Disable any existing waker for this future
        self.core.lock().unwrap().when_target_changed = None;

        // Action depends on the state of the target
        let mut core = self.core.lock().unwrap();
        match &core.target {
            OutputSinkTarget::Disconnected => {
                // Wait for the target to change
                core.when_target_changed = Some(context.waker().clone());
                Poll::Pending
            },

            OutputSinkTarget::Discard => {
                // Throw away any waiting message and say we're done
                mem::drop(core);
                if let Some(when_message_sent) = self.when_message_sent.take() { when_message_sent.wake(); }
                self.waiting_message = None;
                Poll::Ready(Ok(()))
            },

            OutputSinkTarget::Input(input_core)             |
            OutputSinkTarget::CloseWhenDropped(input_core)  => {
                // Try to send to the attached core
                if let Some(input_core) = input_core.upgrade() {
                    mem::drop(core);

                    if let Some(message) = self.waiting_message.take() {
                        // Try sending the waiting message
                        let mut input_core = input_core.lock().unwrap();

                        match input_core.send(self.program_id, message) {
                            Ok(waker) => {
                                // Sent the message: wake up anything waiting for the input stream
                                self.waiting_message = None;
                                mem::drop(input_core);

                                if let Some(waker) = waker { waker.wake() };
                                if let Some(when_message_sent) = self.when_message_sent.take() { when_message_sent.wake(); }
                                Poll::Ready(Ok(()))
                            }

                            Err(message) => {
                                // Need to wait for a slot in the stream
                                self.waiting_message        = Some(message);
                                input_core.wake_when_slots_available(context);

                                mem::drop(input_core);
                                self.core.lock().unwrap().when_target_changed = Some(context.waker().clone());
                                Poll::Pending
                            }
                        }
                    } else {
                        // No message is waiting
                        Poll::Ready(Ok(()))
                    }
                } else {
                    // Core has been released, so we wait as if disconnected
                    core.when_target_changed = Some(context.waker().clone());
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

    impl<TMessage> OutputSink<TMessage> {
        ///
        /// Creates a new output sink that belongs to the specified sub-program
        ///
        pub (crate) fn new(program_id: SubProgramId) -> OutputSink<TMessage> {
            let core = OutputSinkCore {
                target:                 OutputSinkTarget::Disconnected,
                when_target_changed:    None,
            };

            OutputSink {
                program_id:             program_id,
                core:                   Arc::new(Mutex::new(core)),
                waiting_message:        None,
                when_message_sent:      None,
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
            self.core.lock().unwrap().target = OutputSinkTarget::Input(Arc::downgrade(input_stream_core));

            // Wake anything waiting for the stream to become ready or to send a message
            let waker = self.core.lock().unwrap().when_target_changed.take();
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    #[test]
    fn send_message_to_input_stream() {
        // Create an input stream and an output sink
        let program_id          = SubProgramId::new();
        let mut input_stream    = InputStream::<u32>::new(1000);
        let mut output_sink     = OutputSink::new(program_id);

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
        let mut input_stream    = InputStream::<u32>::new(1000);
        let mut output_sink_1   = OutputSink::new(program_id);
        let mut output_sink_2   = OutputSink::new(program_id);

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
        let mut input_stream    = InputStream::<u32>::new(0);
        let mut output_sink     = OutputSink::new(program_id);

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
        let mut input_stream    = InputStream::<u32>::new(0);
        let mut output_sink     = OutputSink::new(program_id);

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
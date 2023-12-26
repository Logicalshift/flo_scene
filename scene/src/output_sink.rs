use crate::{input_stream::*, SubProgramId};

use futures::prelude::*;
use futures::task::{Poll};

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
}

impl<TMessage> OutputSink<TMessage> {
    ///
    /// Creates a new output sink that belongs to the specified sub-program
    ///
    pub (crate) fn new(program_id: SubProgramId) -> OutputSink<TMessage> {
        OutputSink {
            identifier:         NEXT_IDENTIFIER.fetch_add(1, Ordering::Relaxed),
            program_id:         program_id,
            target:             OutputSinkTarget::Disconnected,
            waiting_message:    None,
        }
    }
}

impl<TMessage> Sink<TMessage> for OutputSink<TMessage> 
where
    TMessage: Unpin,
{
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _context: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Always say that we're ready (we store the message in the sink while we're flushing instead)
        match &self.target {
            OutputSinkTarget::Disconnected  => Poll::Pending,
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
        match &self.target {
            OutputSinkTarget::Disconnected  => Poll::Pending,
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
                                self.waiting_message = Some(message);
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

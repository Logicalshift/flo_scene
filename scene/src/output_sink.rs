use crate::error::*;
use crate::input_stream::*;
use crate::scene_context::*;
use crate::scene_core::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::task::{Poll, Waker};

use std::pin::*;
use std::sync::*;

// TODO: close the sink when the target program finishes

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

    /// True if the message was sent by waking the target (we'll return Poll::Pending to yield to the target)
    yield_after_sending: bool,

    /// Waker that is notified when a pending message is sent
    when_message_sent: Option<Waker>,
}

impl<TMessage> Clone for OutputSinkTarget<TMessage> {
    #[inline]
    fn clone(&self) -> Self {
        use OutputSinkTarget::*;

        match self {
            Disconnected                => Disconnected,
            Discard                     => Discard,
            Input(input)                => Input(Weak::clone(input)),
            CloseWhenDropped(input)     => Input(Weak::clone(input)),           // Only the original output sink target will close when dropped
        }
    }
}

impl<TMessage> Drop for OutputSinkTarget<TMessage> {
    #[allow(clippy::single_match)]      // May be more cases in the future, current singleton is not inherent
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

    ///
    /// Returns the ID of the target of this core
    ///
    pub fn target_program_id(core: &Arc<Mutex<Self>>) -> Option<SubProgramId> {
        let input_core = match &core.lock().unwrap().target {
            OutputSinkTarget::Disconnected      | OutputSinkTarget::Discard                         => None,
            OutputSinkTarget::Input(input_core) | OutputSinkTarget::CloseWhenDropped(input_core)    => input_core.upgrade(),
        }?;

        let program_id = input_core.lock().unwrap().target_program_id();
        Some(program_id)
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
            yield_after_sending:    false,
            when_message_sent:      None,
        }
    }

    ///
    /// Retrieves the core of this output snk
    ///
    pub (crate) fn core(&self) -> Arc<Mutex<OutputSinkCore<TMessage>>> {
        Arc::clone(&self.core)
    }

    ///
    /// Sends a message in immediate mode
    ///
    /// If the target input stream supports thread stealing, this may dispatch the message by running that program
    /// immediately. Otherwise, this will queue up the message on the target without blocking regardless of the maximum
    /// depth of the waiting queue. Use `try_send_immediate()` if you have a way to wait for the queue to become free.
    ///
    /// If the stream is disconnected, this will produce the SceneSendError::StreamDisconnected result rather than
    /// blocking until the stream is connected.
    ///
    /// This makes it possible to send messages from functions that are not async. In general, this should be done
    /// sparingly: there's no back-pressure, and this might trigger a future to 'steal' the current thread.
    ///
    pub fn send_immediate(&self, message: TMessage) -> Result<(), SceneSendError> {
        // Try sending the message to the target
        if let Err(message) = self.try_send_immediate(message) {
            // If we can't send it immediately, flush and try again
            self.try_flush_immediate().ok();

            if let Err(message) = self.try_send_immediate(message) {
                // If we still can't send the message, overfill the target buffer
                let source = self.program_id;
                let target = self.core.lock().unwrap().target.clone();

                match &target {
                    OutputSinkTarget::Discard                   => Ok(()),
                    OutputSinkTarget::Disconnected              => Err(SceneSendError::StreamDisconnected),
                    OutputSinkTarget::Input(input)              |
                    OutputSinkTarget::CloseWhenDropped(input)   => {
                        if let Some(input) = input.upgrade() {
                            let waker = input.lock().unwrap().send_with_overfill(source, message)?;
                            if let Some(waker) = waker {
                                waker.wake();
                            }

                            Ok(())
                        } else {
                            Err(SceneSendError::StreamDisconnected)
                        }
                    }
                }
            } else {
                // Sent on the second attempt
                Ok(())
            }
        } else {
            // Initial send worked correctly
            Ok(())
        }
    }

    ///
    /// A variant of send_immediate that fails if the target stream's input buffer is full
    ///
    /// This version of send_immediate does not thread steal, and it also will not over-fill the target buffer.
    /// The message is returned in the error if it was not possible to send it (some action is needed to run
    /// the target future)
    ///
    /// This can be combined with `try_flush_immediate()` to force the messages to process when enough are
    /// buffered.
    ///
    pub fn try_send_immediate(&self, message: TMessage) -> Result<(), TMessage> {
        // Fetch the input core that we'll be sending the message to
        let program_id       = self.program_id;
        let maybe_input_core = match &self.core.lock().unwrap().target {
            OutputSinkTarget::Discard                   => { return Ok(()); },
            OutputSinkTarget::Disconnected              => None,
            OutputSinkTarget::Input(input)              |
            OutputSinkTarget::CloseWhenDropped(input)   => input.upgrade()
        };

        // We're disconnected if the core is 'None'
        if let Some(input_core) = maybe_input_core {
            // Try to enqueue in the input core
            let waker = {
                let mut input_core = input_core.lock().unwrap();

                input_core.send(program_id, message)?
            };

            if let Some(waker) = waker {
                waker.wake();
            }

            Ok(())
        } else {
            Err(message)
        }
    }

    ///
    /// If the target stream allows thread stealing, steal the current thread until the input buffer is empty
    ///
    /// An error result indicates that the target program is already running on the current thread.
    ///
    /// If thread stealing is enabled on the input stream, this will run the target subprogram on the current thread.
    /// If the target program is running on a different thread, this will block the current thread until it is idle.
    ///
    pub fn try_flush_immediate(&self) -> Result<(), SceneSendError> {
        // TODO: we'll probably want to be able to do this from non-scene threads, which requires putting a reference to the scene core in the output stream
        // TODO: an option is to create a separate thread to temporarily run the scene on too, which might work better for processes that can await things 

        // Fetch the scene core to be able to run the process
        let scene_core = scene_context()
            .map(|context| context.scene_core())
            .and_then(|core| core.upgrade())
            .ok_or(SceneSendError::TargetProgramEnded)?;

        // Fetch the input core that's in use
        let maybe_input_core = match &self.core.lock().unwrap().target {
            OutputSinkTarget::Discard                   => None,
            OutputSinkTarget::Disconnected              => None,
            OutputSinkTarget::Input(input)              |
            OutputSinkTarget::CloseWhenDropped(input)   => {
                input.upgrade()
            }
        };

        // Fetch the target program from the input core. This is None if there's no target to flush
        let maybe_target_program_id = maybe_input_core.and_then(|input_core| {
            let input_core = input_core.lock().unwrap();

            if input_core.allows_thread_stealing() {
                Some(input_core.target_program_id())
            } else {
                None
            }
        });

        if let Some(target_program_id) = maybe_target_program_id {
            // Manually poll the process
            // We only poll once, which will empty the queue provided that the target process does not await anything later on
            SceneCore::steal_thread_for_program(&scene_core, target_program_id)?;
        }

        Ok(())
    }
}

impl<TMessage> Sink<TMessage> for OutputSink<TMessage> 
where
    TMessage: Unpin,
{
    type Error = SceneSendError;

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
                OutputSinkTarget::Disconnected => {
                    core.when_target_changed = Some(context.waker().clone());
                    Poll::Pending
                },
                OutputSinkTarget::Discard => Poll::Ready(Ok(())),

                OutputSinkTarget::Input(input_core)               |
                OutputSinkTarget::CloseWhenDropped(input_core)    => {
                    if input_core.upgrade().is_none() {
                        // Downgrade to a disconnected core so the sending can be retried
                        core.target = OutputSinkTarget::Disconnected;

                        // Error if the target program is not running any more
                        Poll::Ready(Err(SceneSendError::TargetProgramEnded))
                    } else {
                        // Can send the message
                        Poll::Ready(Ok(()))
                    }
                }
            }
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: TMessage) -> Result<(), Self::Error> {
        use std::mem;

        self.yield_after_sending = false;

        let mut core = self.core.lock().unwrap();
        match &core.target {
            OutputSinkTarget::Disconnected                  => {
                mem::drop(core);
                self.waiting_message = Some(item);
                Ok(())
            },

            OutputSinkTarget::Discard                       => {
                mem::drop(core);
                if let Some(when_message_sent) = self.when_message_sent.take() { when_message_sent.wake(); }
                self.waiting_message = None;
                Ok(())
            },

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

                            if let Some(waker) = waker {
                                self.yield_after_sending = true;
                                waker.wake()
                            };
                            Ok(())
                        }

                        Err(item) => {
                            // Need to wait for a slot in the stream
                            self.waiting_message = Some(item);
                            Ok(())
                        }
                    }
                } else {
                    // Downgrade to a disconnected core so the sending can be retried
                    core.target = OutputSinkTarget::Disconnected;

                    // Target program is not available
                    Err(SceneSendError::TargetProgramEnded)
                }
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        use std::mem;

        // If 'yield after sending' is set, we return Poll::Pending and immediately wake ourselves up (which will give the target program a chance to run and clear the message)
        if self.yield_after_sending {
            // Unset the 'yield after sending' flag
            self.yield_after_sending = false;

            // Reawaken the future immediately
            context.waker().wake_by_ref();

            // Indicate that we're pending
            return Poll::Pending;
        }

        // If there's no waiting message, then it has been sent and there's no work to do
        if self.waiting_message.is_none() {
            return Poll::Ready(Ok(()));
        }

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
                                if let Some(when_message_sent) = self.when_message_sent.take() { 
                                    when_message_sent.wake();
                                }
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
                    // Downgrade to a disconnected core so the sending can be retried
                    core.target = OutputSinkTarget::Disconnected;

                    // When the core is released during a send, the target program has terminated, so we generate an error
                    core.when_target_changed    = Some(context.waker().clone());
                    Poll::Ready(Err(SceneSendError::TargetProgramEnded))
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
                yield_after_sending:    false,
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
        let mut input_stream    = InputStream::<u32>::new(program_id, 1000);
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
        let mut input_stream    = InputStream::<u32>::new(program_id, 1000);
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
        let mut input_stream    = InputStream::<u32>::new(program_id, 0);
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
        let mut input_stream    = InputStream::<u32>::new(program_id, 0);
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
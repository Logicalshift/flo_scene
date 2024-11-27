use super::poll_result::*;
use super::runtime::*;
use crate::host::error::*;
use crate::host::serialization_context::*;
use crate::util::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use std::sync::*;

///
/// Serialization context used for guest subprograms
///
#[derive(Clone)]
pub (super) struct GuestSerializationContext {
    core:           Weak<Mutex<GuestRuntimeCore>>,
    future_pile:    FuturePile,
}

impl GuestSerializationContext {
    ///
    /// Creates a new serialization context for this guest
    ///
    pub fn new(core: &Arc<Mutex<GuestRuntimeCore>>, pile: &FuturePile) -> Self {
        GuestSerializationContext {
            core:           Arc::downgrade(core),
            future_pile:    pile.clone(),
        }
    }
}

/// Sends a close event for a serialization ID when dropped
struct CloseStream(SerializationId, Weak<Mutex<GuestRuntimeCore>>);

impl Drop for CloseStream {
    fn drop(&mut self) {
        let CloseStream(stream_id, core) = self;

        if let Some(core) = core.upgrade() {
            core.lock().unwrap().pending_results.push(GuestResult::CloseStream(*stream_id));
        }
    }
}

impl SerializationContext for GuestSerializationContext {
    fn send_stream(&self, stream: BoxStream<'static, Vec<u8>>) -> Result<SerializationId, SceneSendError<BoxStream<'static, Vec<u8>>>> {
        if let Some(core) = self.core.upgrade() {
            // Create a serialization ID for this stream
            let stream_id = GuestRuntimeCore::next_serialization_id(&core).to_mine();

            let pile = self.future_pile.clone();

            self.future_pile.add_future(async move {
                // Ensure that the stream is closed if this future is ever dropped
                use std::mem;
                let close_stream = CloseStream(stream_id, Arc::downgrade(&core));

                let mut stream = stream;

                loop {
                    // End state of the polling request
                    enum ReadyMessage {
                        Ready(Option<Vec<u8>>),
                        ClosedOnOtherSide
                    }

                    // Wait for the stream to become ready and a message to be available (or closed, in which case we end this future)
                    let mut poll_message        = Some(stream.next());
                    let mut received_message    = None;
                    let mut busy_with_message   = None;

                    let next_message = future::poll_fn(|ctxt| {
                        use futures::task::*;

                        // Poll for the next message from the stream
                        if received_message.is_none() {
                            if let Poll::Ready(message) = poll_message.as_mut().unwrap().poll_unpin(ctxt) {
                                if message.is_none() {
                                    // Don't need the other side to be ready if the stream is closed
                                    return Poll::Ready(ReadyMessage::Ready(None));
                                }

                                // The source side has generated a message (we can return it once the target side is ready)
                                busy_with_message   = Some(pile.make_busy());
                                received_message    = Some(message);
                                poll_message        = None;
                            }
                        }

                        let mut locked_core = core.lock().unwrap();

                        if locked_core.closed_streams.contains(&stream_id) {
                            // Stream is closed (return true, the stream is closed)
                            Poll::Ready(ReadyMessage::ClosedOnOtherSide)
                        } else if locked_core.ready_streams.contains(&stream_id) {
                            // Stream is ready (return false, the stream is not closed)
                            if let Some(message) = received_message.take() {
                                // Received a message from the source stream and the other side is ready to receive it
                                locked_core.ready_streams.remove(&stream_id);
                                Poll::Ready(ReadyMessage::Ready(message))
                            } else {
                                // Waiting for the next message to arrive from the stream
                                Poll::Pending
                            }
                        } else {
                            // Stream is not ready, wait for it
                            locked_core.when_ready.insert(stream_id, Some(ctxt.waker().clone()));
                            Poll::Pending
                        }
                    }).await;

                    // We can either get a message that's ready to send to the other side, or the other side can indicate we're closed, or our own side can close the stream
                    match next_message {
                        ReadyMessage::ClosedOnOtherSide => {
                            // Stop when the stream is closed
                            break;
                        }

                        ReadyMessage::Ready(None) => {
                            // No more messages from the source stream
                            break;
                        }

                        ReadyMessage::Ready(Some(next_message)) => {
                            // Send as an action to the host (the host will re-ready the stream after this event)
                            // These messages are picked up later on by the callback from the host
                            core.lock().unwrap().pending_results.push(GuestResult::SendStream(stream_id, next_message));
                        }
                    }

                    // Message is sent, so we're not busy any more
                    mem::drop(busy_with_message);
                }

                // Remove the status for this stream before shutting down
                let mut locked_core = core.lock().unwrap();

                locked_core.closed_streams.remove(&stream_id);
                locked_core.ready_streams.remove(&stream_id);
                locked_core.when_ready.remove(&stream_id);

                mem::drop(locked_core);
                mem::drop(close_stream);
            });

            // Serialize the stream to be sent to the guest as 'theirs' (we always assume it'll be sent over the connection)
            Ok(stream_id.to_theirs())
        } else {
            Err(SceneSendError::TargetProgramEndedBeforeReady)
        }
    }

    fn receive_stream(&self, stream_id: SerializationId) -> Result<BoxStream<'static, Vec<u8>>, SceneSendError<SerializationId>> {
        if let Some(core) = self.core.upgrade() {
            // Create a core for this stream
            let stream_core = GuestRuntimeCore::create_stream_from_host(&core, stream_id);

            // This is dropped when the stream is finished with and generates a 'close stream' event
            let close_stream = CloseStream(stream_id, Arc::downgrade(&core));

            // Use an unfold to generate the messages for the stream
            let stream = stream::unfold((core, stream_core, close_stream), move |(core, stream_core, close_stream)| async move { 
                // Stream is ready to receive a message
                core.lock().unwrap().pending_results.push(GuestResult::ReadyStream(stream_id));

                // Wait for a message to arrive
                let next_msg = future::poll_fn(|ctxt| {
                    use futures::task::*;

                    let mut stream_core = stream_core.lock().unwrap();

                    if let Some(msg) = stream_core.pending.pop_front() {
                        // There's a message waiting
                        Poll::Ready(Some(msg))
                    } else if stream_core.closed {
                        // No messages and the stream is closed
                        Poll::Ready(None)
                    } else {
                        // Sleep until a message is received
                        stream_core.waker = Some(ctxt.waker().clone());
                        Poll::Pending
                    }
                }).await;

                // Return messages until the stream is closed
                match next_msg {
                    Some(msg) => Some((msg, (core, stream_core, close_stream))),
                    None      => None,
                }
            });

            Ok(stream.boxed())
        } else {
            // Core is no longer running
            Err(SceneSendError::TargetProgramEndedBeforeReady)
        }
    }

    fn send_function(&self, callback: RemoteCallbackFn) -> Result<SerializationId, SceneSendError<RemoteCallbackFn>> {
        todo!()
    }

    fn receive_function(&self, callback_id: SerializationId) -> Result<RemoteCallbackFn, SceneSendError<SerializationId>> {
        todo!()
    }
}

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

impl SerializationContext for GuestSerializationContext {
    fn send_stream(&self, stream: BoxStream<'static, Vec<u8>>) -> Result<SerializationId, SceneSendError<BoxStream<'static, Vec<u8>>>> {
        if let Some(core) = self.core.upgrade() {
            // Create a serialization ID for this stream
            let stream_id = GuestRuntimeCore::next_serialization_id(&core).to_mine();

            // Add a future to the pile to follow the stream and send messages via the core
            let pile = core.lock().unwrap().future_pile.clone();

            pile.add_future(async move {
                let mut stream = stream;

                loop {
                    // Wait for the stream to become ready (or closed, in which case we end this future)
                    let is_closed = future::poll_fn(|ctxt| {
                        use futures::task::*;

                        let mut locked_core = core.lock().unwrap();

                        if locked_core.closed_streams.contains(&stream_id) {
                            // Stream is closed (return true, the stream is closed)
                            Poll::Ready(true)
                        } else if locked_core.ready_streams.contains(&stream_id) {
                            // Stream is ready (return false, the stream is not closed)
                            locked_core.ready_streams.remove(&stream_id);
                            Poll::Ready(false)
                        } else {
                            // Stream is not ready, wait for it
                            locked_core.when_ready.insert(stream_id, Some(ctxt.waker().clone()));
                            Poll::Pending
                        }
                    }).await;

                    // Stop when the stream is closed
                    if is_closed {
                        break;
                    }

                    // Receive a message from the source stream
                    let next_message = stream.next().await;

                    if let Some(next_message) = next_message {
                        // Send as an action to the host (the host will re-ready the stream after this event)
                        // These messages are picked up later on by the callback from the host
                        core.lock().unwrap().pending_results.push(GuestResult::SendStream(stream_id, next_message));
                    } else {
                        // No more messages in the stream
                        break;
                    }
                }

                // Remove the status for this stream before shutting down
                let mut locked_core = core.lock().unwrap();

                locked_core.closed_streams.remove(&stream_id);
                locked_core.ready_streams.remove(&stream_id);
                locked_core.when_ready.remove(&stream_id);
            });

            // Serialize the stream to be sent to the guest as 'theirs'
            Ok(stream_id)
        } else {
            Err(SceneSendError::TargetProgramEndedBeforeReady)
        }
    }

    fn receive_stream(&self, stream_id: SerializationId) -> Result<BoxStream<'static, Vec<u8>>, SceneSendError<SerializationId>> {
        if let Some(core) = self.core.upgrade() {
            // Create a core for this stream
            let stream_core = GuestRuntimeCore::create_stream_from_host(&core, stream_id);

            // Use an unfold to generate the messages for the stream
            let stream = stream::unfold((core, stream_core), move |(core, stream_core)| async move { 
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
                    Some(msg) => Some((msg, (core, stream_core))),
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

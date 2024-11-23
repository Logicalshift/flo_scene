use super::guest_stream_core::*;
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
        todo!()
    }

    fn receive_stream(&self, stream_id: SerializationId) -> Result<BoxStream<'static, Vec<u8>>, SceneSendError<SerializationId>> {
        if let Some(core) = self.core.upgrade() {
            // Create a core for this stream
            let stream_core = GuestRuntimeCore::create_stream_from_host(&core, stream_id);

            // Use an unfold to generate the messages for the stream
            let stream = stream::unfold(stream_core, move |stream_core| async move { 
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
                    Some(msg) => Some((msg, stream_core)),
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

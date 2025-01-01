use crate::guest_types::*;
use super::core::*;
use super::input_stream_core::*;
use super::serialization_context::*;

use futures::prelude::*;
use futures::task::{Context, Poll};

use core::marker::{PhantomData};
use alloc::collections::{VecDeque};

///
/// A guest input stream works with the reads deserialized messages from the host side
///
pub struct GuestInputStream<TMessageType: SceneGuestMessage> {
    /// The core is shared with the runtime for managing the input stream
    core: Shared<GuestInputStreamCore>,

    /// The handle assigned to the subprogram that owns this input stream
    program_handle: GuestSubProgramHandle,

    /// The runtime core (we need this to signal 'ready')
    runtime_core: Shared<GuestRuntimeCore>,

    /// The serialization context to use when decoding messages
    serialization_context: GuestSerializationContext,

    /// Phantom data, what the waiting messages are decoded as
    decode_as: PhantomData<TMessageType>,
}

impl<TMessageType> GuestInputStream<TMessageType>
where
    TMessageType: SceneGuestMessage,
{
    /// Creates a new guest input stream
    pub (super) fn new(program_handle: GuestSubProgramHandle, runtime_core: &Shared<GuestRuntimeCore>, serialization_context: GuestSerializationContext) -> Self {
        // Create the core
        let core = GuestInputStreamCore {
            waiting:    VecDeque::new(),
            waker:      None,
            closed:     false,
            is_ready:   false,
        };
        let core            = share(core);
        let runtime_core    = runtime_core.clone();
        let decode_as       = PhantomData;

        Self { core, program_handle, runtime_core, decode_as, serialization_context }
    }

    /// Retrieves the core of this input stream
    #[inline]
    pub (crate) fn core(&self) -> &Shared<GuestInputStreamCore> {
        &self.core
    }
}

impl<TMessageType> Stream for GuestInputStream<TMessageType> 
where
    TMessageType: SceneGuestMessage,
{
    type Item = TMessageType;

    fn poll_next(self: core::pin::Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Read the encoded form of the next message from the core
        let mut signal_ready    = false;
        let next_message        = with_shared(&self.core, |core| {
            if let Some(encoded) = core.waiting.pop_front() {
                // There's a message waiting
                core.is_ready = false;
                Poll::Ready(Some(encoded))
            } else if core.closed {
                // Stream has finished
                Poll::Ready(None)
            } else {
                // Stream is blocked: store the waker so we can invoke this in the future
                core.waker = Some(context.waker().clone());

                if !core.is_ready {
                    // The core is ready
                    core.is_ready = true;

                    // Signal via the runtime
                    signal_ready = true;

                    Poll::Pending
                } else {
                    Poll::Pending
                }
            }
        });

        // Signal that the stream is ready once we've dropped the lock
        if signal_ready {
            GuestRuntimeCore::stream_ready(&self.runtime_core, self.program_handle);
        }

        // Decode the message
        match next_message {
            Poll::Pending               => Poll::Pending,
            Poll::Ready(None)           => Poll::Ready(None),
            Poll::Ready(Some(bytes))    => {
                if let Ok(msg) = TMessageType::from_guest_message(&bytes, &self.serialization_context) {
                    Poll::Ready(Some(msg))
                } else {
                    Poll::Ready(None)
                }
            },
        }
    }
}

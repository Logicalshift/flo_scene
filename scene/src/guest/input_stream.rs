use super::guest_message::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{Waker, Poll, Context};

use std::collections::{VecDeque};
use std::marker::{PhantomData};
use std::sync::*;

///
/// The input stream core is used in
///
pub (crate) struct GuestInputStreamCore {
    /// Messages waiting in this input stream
    waiting: VecDeque<Vec<u8>>,

    /// Waker for the future for this input stream
    waker: Option<Waker>,

    /// Set to true once the stream should be considered to be closed
    closed: bool,
}

///
/// A guest input stream works with the reads deserialized messages from the host side
///
pub struct GuestInputStream<TMessageType: GuestSceneMessage> {
    /// The core is shared with the runtime for managing the input stream
    core: Arc<Mutex<GuestInputStreamCore>>,

    /// The decoder turns an encoded message back into a TMessageType
    decoder: Box<dyn 'static + Send + Fn(Vec<u8>) -> TMessageType>,

    /// Phantom data, what the waiting messages are decoded as
    decode_as: PhantomData<TMessageType>,
}


impl<TMessageType> Stream for GuestInputStream<TMessageType> 
where
    TMessageType: GuestSceneMessage,
{
    type Item = TMessageType;

    fn poll_next(self: std::pin::Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Read the encoded form of the next message from the core
        let next_message = {
            let mut core = self.core.lock().unwrap();

            if let Some(encoded) = core.waiting.pop_front() {
                // There's a message waiting
                Poll::Ready(Some(encoded))
            } else if core.closed {
                // Stream has finished
                Poll::Ready(None)
            } else {
                // Stream is blocked: store the waker so we can invoke this in the future
                core.waker = Some(context.waker().clone());
                Poll::Pending
            }
        };

        // Decode the message
        match next_message {
            Poll::Pending               => Poll::Pending,
            Poll::Ready(None)           => Poll::Ready(None),
            Poll::Ready(Some(bytes))    => Poll::Ready(Some((self.decoder)(bytes))),
        }
    }
}

use crate::{SubProgramId};

use futures::prelude::*;
use futures::task::{Waker, Poll};

use std::collections::*;
use std::sync::*;

///
/// The input stream core is a shareable part of an input stream for a program
///
pub (crate) struct InputStreamCore<TMessage> {
    /// The subprogram that this stream belongs to
    program_id: SubProgramId,

    /// The maximum number of waiting messages for this input stream
    max_waiting: usize,

    /// Messages waiting to be delivered
    waiting_messages: VecDeque<TMessage>,

    /// A waker for the future that is waiting for the next message in this stream
    when_message_sent: Option<Waker>,

    /// Wakers for any output streams waiting for slots to become available
    when_slots_available: VecDeque<Waker>,

    /// True if this stream is closed (because the subprogram is ending)
    closed: bool,
}

///
/// An input stream for a subprogram
///
pub struct InputStream<TMessage> {
    core: Arc<Mutex<InputStreamCore<TMessage>>>,
}

impl<TMessage> InputStream<TMessage> {
    ///
    /// Creates a new input stream
    ///
    pub (crate) fn new(program_id: SubProgramId, max_waiting: usize) -> Self {
        let core = InputStreamCore {
            program_id:             program_id,
            max_waiting:            max_waiting,
            waiting_messages:       VecDeque::new(),
            when_message_sent:      None,
            when_slots_available:   VecDeque::new(),
            closed:                 false,
        };

        InputStream {
            core: Arc::new(Mutex::new(core))
        }
    }
}

impl<TMessage> InputStreamCore<TMessage> {
    ///
    /// Adds a message to this core if there's space for it
    ///
    pub (crate) fn send(&mut self, message: TMessage) -> Result<(), TMessage> {
        if self.waiting_messages.len() <= self.max_waiting {
            self.waiting_messages.push_back(message);
            Ok(())
        } else {
            Err(message)
        }
    }
}

impl<TMessage> Stream for InputStream<TMessage> {
    type Item=TMessage;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        use std::mem;

        let mut core = self.core.lock().unwrap();

        if let Some(message) = core.waiting_messages.pop_front() {
            // If any of the output sinks are waiting to write a value, wake them up as the queue has reduced
            let next_available = core.when_slots_available.pop_front();
            mem::drop(core);

            if let Some(next_available) = next_available {
                next_available.wake()
            }

            // Return the message
            Poll::Ready(Some(message))
        } else if core.closed {
            // Once all the messages are delivered and the core is closed, close the stream
            Poll::Ready(None)
        } else {
            // Wait for the next message to be delivered
            core.when_message_sent = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

use crate::{SubProgramId};

use futures::prelude::*;
use futures::task::{Waker, Poll, Context};

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
    pub (crate) core: Arc<Mutex<InputStreamCore<TMessage>>>,
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
    /// Adds a message to this core if there's space for it, returning the waker to be called if successful (the waker must be called with the core unlocked)
    ///
    pub (crate) fn send(&mut self, message: TMessage) -> Result<Option<Waker>, TMessage> {
        if self.waiting_messages.len() <= self.max_waiting {
            self.waiting_messages.push_back(message);
            Ok(self.when_message_sent.take())
        } else {
            Err(message)
        }
    }

    ///
    /// Wakes the future specified by a context as soon as a slot becomes available
    ///
    pub (crate) fn wake_when_slots_available(&mut self, context: &mut Context) {
        self.when_slots_available.push_back(context.waker().clone());
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

            // Release the core lock before waking anything
            mem::drop(core);

            next_available.into_iter().for_each(|waker| waker.wake());

            // Return the message
            Poll::Ready(Some(message))
        } else if core.closed {
            // Once all the messages are delivered and the core is closed, close the stream
            Poll::Ready(None)
        } else {
            // Wait for the next message to be delivered
            core.when_message_sent = Some(cx.waker().clone());

            // Don't go to sleep until everything that's waiting for a slot has been woken up
            let next_available = core.when_slots_available.drain(..).collect::<Vec<_>>();

            // Release the core lock before waking anything
            mem::drop(core);

            next_available.into_iter().for_each(|waker| waker.wake());

            Poll::Pending
        }
    }
}

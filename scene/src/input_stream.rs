use crate::error::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::task::{Waker, Poll, Context};

use std::collections::*;
use std::sync::*;

// TODO: add a way to read a message along with its source

///
/// The input stream core is a shareable part of an input stream for a program
///
pub (crate) struct InputStreamCore<TMessage> {
    /// The program that owns this input stream
    program_id: SubProgramId,

    /// The maximum number of waiting messages for this input stream
    max_waiting: usize,

    /// Messages waiting to be delivered
    waiting_messages: VecDeque<(SubProgramId, TMessage)>,

    /// A waker for the future that is waiting for the next message in this stream
    when_message_sent: Option<Waker>,

    /// Wakers for any output streams waiting for slots to become available
    when_slots_available: VecDeque<Waker>,

    /// If non-zero, this input stream is blocked from receiving any more data (even if slots are waiting in max-waiting), creating back-pressure on anything that's outputting to it 
    blocked: usize,

    /// True if this stream is closed (because the subprogram is ending)
    closed: bool,
}

/// A struct that unblocks an input stream when dropped
pub struct BlockedStream<TMessage>(Weak<Mutex<InputStreamCore<TMessage>>>);

///
/// An input stream blocker is used to disable input to an input stream temporarily
///
/// As a separate object, this allows blocking of its source stream even when that stream's object is not directly
/// available (eg, if you call `input_stream.map(...)`, direct access to the stream is no longer available, but it
/// can still be blocked if one of these was created)
///
/// Blocks are returned as a `BlockedStream` object, which will unblock the stream when it is disposed.
///
#[derive(Clone)]
pub struct InputStreamBlocker<TMessage>(Weak<Mutex<InputStreamCore<TMessage>>>);

///
/// An input stream for a subprogram
///
pub struct InputStream<TMessage> {
    pub (crate) core: Arc<Mutex<InputStreamCore<TMessage>>>,
}

///
/// An input stream for a subprogram, which returns the source of each message
///
struct InputStreamWithSources<TMessage> {
    pub (crate) core: Arc<Mutex<InputStreamCore<TMessage>>>,
}

impl<TMessage> Drop for BlockedStream<TMessage> {
    fn drop(&mut self) {
        use std::mem;

        if let Some(core) = self.0.upgrade() {
            // Reduce the blocked count
            let mut core = core.lock().unwrap();
            core.blocked -= 1;

            // Wake the core if it has become unblocked
            if core.blocked == 0 {
                // Core is unblocked: take anything that's waiting for slots, then unlock the core
                let when_slots_available = core.when_slots_available.drain(..).collect::<Vec<_>>();
                mem::drop(core);

                // Wake everything that's waiting for this input stream to unblock
                when_slots_available.into_iter()
                    .for_each(|waker| waker.wake());
            }
        }
    }
}

impl<TMessage> InputStreamBlocker<TMessage> {
    ///
    /// Blocks the input stream, preventing any further input
    ///
    pub fn block(&self) -> BlockedStream<TMessage> {
        // Increase the block count in the core (it won't accept future messages)
        if let Some(core) = self.0.upgrade() {
            let mut core = core.lock().unwrap();
            core.blocked += 1;
        }

        // Return an object that will unblock the stream when it is dropped
        BlockedStream(self.0.clone())
    }
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
            blocked:                0,
            closed:                 false,
        };

        InputStream {
            core: Arc::new(Mutex::new(core))
        }
    }

    ///
    /// Fetches the core of this stream
    ///
    pub (crate) fn core(&self) -> Arc<Mutex<InputStreamCore<TMessage>>> {
        Arc::clone(&self.core)
    }

    ///
    /// Upgrades this stream to return the messages with the source subprogram IDs
    ///
    pub fn messages_with_sources(self) -> impl Stream<Item=(SubProgramId, TMessage)> {
        InputStreamWithSources {
            core: self.core
        }
    }

    ///
    /// Returns an object that can be used to block this stream
    ///
    #[inline]
    pub fn blocker(&self) -> InputStreamBlocker<TMessage> {
        InputStreamBlocker(Arc::downgrade(&self.core))
    }

    ///
    /// Enables thread stealing for subprograms that want to use this input stream for immediate output
    ///
    /// If thread stealing is enabled, then if `OutputSink::send_immediate()` is called the future that is waiting
    /// on the input stream will be polled in immediate mode until the message is processed. This is non-standard
    /// for Rust futures, but enables things like loggers to produce their output immediately instead of needing
    /// to wait for the main event loop to trigger. The future will run on whatever context is active on the thread
    /// that send_immediate is called on, so must not be dependent on the scene's own context to be completely
    /// safe.
    ///
    /// If thread stealing is disabled, then `send_immediate` may overload the input to the stream in order to 
    /// avoid needing to block the thread (as that might just deadlock).
    ///
    #[inline]
    pub fn allow_thread_stealing(&self, enable: bool) {
        todo!()
    }
}

impl<TMessage> InputStreamCore<TMessage> {
    ///
    /// Adds a message to this core if there's space for it, returning the waker to be called if successful (the waker must be called with the core unlocked)
    ///
    pub (crate) fn send(&mut self, source: SubProgramId, message: TMessage) -> Result<Option<Waker>, TMessage> {
        if self.blocked == 0 && self.waiting_messages.len() <= self.max_waiting {
            // The input stream is not blocked and has space in the waiting_messages queue for this event: queue it up and return the waker
            self.waiting_messages.push_back((source, message));
            Ok(self.when_message_sent.take())
        } else {
            // The input stream is blocked or the queue is full: return the message to sender
            Err(message)
        }
    }

    ///
    /// Adds a message to the queue for this core even if the max waiting size has been exceeded
    ///
    /// This is used for forcibly sending messages in immediate mode to guarantee delivery (and can result in memory leaks)
    ///
    pub (crate) fn send_with_overfill(&mut self, source: SubProgramId, message: TMessage) -> Result<Option<Waker>, SceneSendError> {
        if self.closed {
            Err(SceneSendError::StreamDisconnected)
        } else {
            self.waiting_messages.push_back((source, message));
            Ok(self.when_message_sent.take())
        }
    }

    ///
    /// Wakes the future specified by a context as soon as a slot becomes available
    ///
    pub (crate) fn wake_when_slots_available(&mut self, context: &mut Context) {
        self.when_slots_available.push_back(context.waker().clone());
    }

    ///
    /// Returns the size of the buffer that this stream allows
    ///
    pub (crate) fn num_slots(&self) -> usize {
        self.max_waiting
    }

    ///
    /// Sets this stream as 'closed' (which generally stops the process from running any further)
    ///
    pub (crate) fn close(&mut self) -> Option<Waker> {
        self.closed = true;
        self.when_message_sent.take()
    }

    ///
    /// Retrieves the program ID that owns this input stream
    ///
    pub (crate) fn target_program_id(&self) -> SubProgramId {
        self.program_id
    }
}

impl<TMessage> Stream for InputStream<TMessage> {
    type Item=TMessage;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        use std::mem;

        let mut core = self.core.lock().unwrap();

        if let Some((_source, message)) = core.waiting_messages.pop_front() {
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

impl<TMessage> Stream for InputStreamWithSources<TMessage> {
    type Item=(SubProgramId, TMessage);

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        use std::mem;

        let mut core = self.core.lock().unwrap();

        if let Some(message_and_source) = core.waiting_messages.pop_front() {
            // If any of the output sinks are waiting to write a value, wake them up as the queue has reduced
            let next_available = core.when_slots_available.pop_front();

            // Release the core lock before waking anything
            mem::drop(core);

            next_available.into_iter().for_each(|waker| waker.wake());

            // Return the message
            Poll::Ready(Some(message_and_source))
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

use crate::error::*;
use crate::scene_message::*;
use crate::scene_core::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::task::{Waker, Poll, Context};

use std::collections::*;
use std::sync::*;

///
/// The input stream core is a shareable part of an input stream for a program
///
pub (crate) struct InputStreamCore<TMessage> {
    /// The program that owns this input stream
    program_id: SubProgramId,

    /// The maximum number of waiting messages for this input stream
    max_waiting: usize,

    /// The maximum number of wiating messages while this is waiting to become idle
    max_idle_queue_len: usize,

    /// The scene that this input is a part of
    scene_core: Weak<Mutex<SceneCore>>,

    /// Messages waiting to be delivered
    waiting_messages: VecDeque<(SubProgramId, TMessage)>,

    /// A waker for the future that is waiting for the next message in this stream
    when_message_sent: Option<Waker>,

    /// Wakers for any output streams waiting for slots to become available
    when_slots_available: VecDeque<Waker>,

    /// True if immediate-mode requests are allowed to steal the current thread (false if this can only be run from the main scene loop)
    allow_thread_stealing: bool,

    /// If non-zero, this input stream is blocked from receiving any more data (even if slots are waiting in max-waiting), creating back-pressure on anything that's outputting to it 
    blocked: usize,

    /// True if this stream is closed (because the subprogram is ending)
    closed: bool,

    /// True if this stream has been polled while empty, false if this stream has recently returned a value
    idle: bool,

    /// The number of times the owner of this input stream is waiting for the scene to become idle
    waiting_for_idle: usize,
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

    /// Set to false if the core is transferred elsewhere (the core won't be closed when this is dropped)
    active: bool,
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

impl<TMessage> InputStream<TMessage> 
where
    TMessage: SceneMessage,
{
    ///
    /// Creates a new input stream
    ///
    pub (crate) fn new(program_id: SubProgramId, scene_core: &Arc<Mutex<SceneCore>>, max_waiting: usize) -> Self {
        let core = InputStreamCore {
            program_id:             program_id,
            max_waiting:            max_waiting,
            max_idle_queue_len:     max_waiting,
            scene_core:             Arc::downgrade(scene_core),
            waiting_messages:       VecDeque::new(),
            when_message_sent:      None,
            when_slots_available:   VecDeque::new(),
            blocked:                0,
            allow_thread_stealing:  TMessage::allow_thread_stealing_by_default(),
            closed:                 false,
            idle:                   false,
            waiting_for_idle:       0,
        };

        InputStream {
            core:   Arc::new(Mutex::new(core)),
            active: true,
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
    pub fn messages_with_sources(mut self) -> impl Stream<Item=(SubProgramId, TMessage)> {
        self.active = false;

        InputStreamWithSources {
            core: self.core.clone(),
        }
    }

    ///
    /// Returns an object that can be used to block this stream
    ///
    /// Calling `block()` on the struct that this returns will prevent any messages from being queued on this
    /// stream until it has been unblocked. This can be used to generate back-pressure to anything that is
    /// trying to send messages to this stream. The messages will be blocked even if there's space in the input
    /// queue for this stream.
    ///
    #[inline]
    pub fn blocker(&self) -> InputStreamBlocker<TMessage> {
        InputStreamBlocker(Arc::downgrade(&self.core))
    }

    ///
    /// Sets whether or not thread stealing is enabled for subprograms that want to use this input stream for immediate output
    ///
    /// Thread stealing is turned off by default. Turning it on can result in sent messages being processed immediately
    /// rather than waiting for the future to yield by immediately running the target program when a message is sent.
    /// This is also useful for handling messages sent with `send_immediate`. 
    ///
    /// Deciding whether to use this requires some knowledge of how futures work. You will get the best effect if the 
    /// subprogram does not await anything between processing messages, or at least does not await anything in the usual 
    /// case. In general, leave this disabled if there's no specific need for it.
    ///
    /// Normally, when a future awaits the effect is to return the thread to the main loop, which in the case of flo_scene
    /// switches to the oldest waiting future. Sending a message will await once the input queue is full (once there are
    /// `max_waiting` messages waiting). This works fine most of the time, but for some message types - like for example
    /// logging - the delay in processing the message can be undesirable, and this can also result in a backlog when
    /// sending messages in immediate mode.
    ///
    /// With thread stealing enabled, sending a message will immediately poll the target program. Provided that the program
    /// only awaits new messages, this means that the message will be processed right away with no other futures running.
    /// This is quite useful for certain tasks like logging where having the message process at the exact time it is sent
    /// is a needed feature. However, if the target is awaiting some other future at the time the message is sent, this can
    /// just result in no effect.
    ///
    #[inline]
    pub fn allow_thread_stealing(&self, enable: bool) {
        self.core.lock().unwrap().allow_thread_stealing = enable;
    }
}

impl<TMessage> InputStreamCore<TMessage> 
where
    TMessage: 'static + Send,
{
    ///
    /// Retrieves the scene core for this input stream if there is one
    ///
    pub (crate) fn scene_core(&self) -> Option<Arc<Mutex<SceneCore>>> {
        self.scene_core.upgrade()
    }

    ///
    /// Adds a message to this core if there's space for it, returning the waker to be called if successful (the waker must be called with the core unlocked)
    ///
    pub (crate) fn send(&mut self, source: SubProgramId, message: TMessage) -> Result<Option<Waker>, TMessage> {
        if !self.closed && self.blocked == 0 && self.waiting_messages.len() <= self.max_waiting {
            // The input stream is not blocked and has space in the waiting_messages queue for this event: queue it up and return the waker
            self.waiting_messages.push_back((source, message));
            self.idle = false;
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
    pub (crate) fn send_with_overfill(&mut self, source: SubProgramId, message: TMessage) -> Result<Option<Waker>, SceneSendError<TMessage>> {
        if self.closed {
            Err(SceneSendError::StreamDisconnected(message))
        } else {
            self.waiting_messages.push_back((source, message));
            self.idle = false;
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
    /// The stream can queue up one more message than this number: this is the number of messages this input stream
    /// can buffer before the sender needs to yield.
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

    ///
    /// True if this input stream can 'steal' the current thread to send messages immediately
    ///
    pub (crate) fn allows_thread_stealing(&self) -> bool {
        self.allow_thread_stealing
    }

    ///
    /// True if the queue for this input stream is full (the next `send()` call will fail)
    ///
    pub (crate) fn is_queue_full(&self) -> bool {
        (self.max_waiting + 1) <= self.waiting_messages.len()
    }

    ///
    /// True if this input core has been closed
    ///
    pub (crate) fn is_closed(&self) -> bool {
        self.closed
    }

    ///
    /// True if this input stream is blocked and shouldn't accept any more messages
    ///
    pub (crate) fn is_blocked(&self) -> bool {
        self.blocked > 0
    }

    ///
    /// True if this input stream is idle (has no waiting messages and is being waiting upon)
    ///
    #[inline]
    pub (crate) fn is_idle(&self) -> bool {
        (self.idle && self.waiting_messages.is_empty()) || (self.waiting_for_idle > 0)
    }

    ///
    /// Marks this input stream as 'waiting for idle', where it will accept messages but won't block other idle notifications from firing
    ///
    /// Senders will receive an error instead of backpressure if `max_idle_queue_len` is exceeded, otherwise, this core will be able to queue
    /// up to max_idle_queue_len messages while it waits for the core to become idle.
    ///
    pub (crate) fn waiting_for_idle(core: &Arc<Mutex<Self>>, max_idle_queue_len: usize) -> IdleInputStreamCore {
        {
            // Mark the core are 
            let mut core = core.lock().unwrap();

            core.waiting_for_idle += 1;
            core.max_idle_queue_len = core.max_idle_queue_len.max(max_idle_queue_len);
        }

        let core = Arc::downgrade(core);
        let idle_dropper = IdleInputStreamCore(Some(Box::new(move ||
                if let Some(core) = core.upgrade() {
                    let mut core = core.lock().unwrap();

                    core.waiting_for_idle -= 1;
                    if core.waiting_for_idle == 0 {
                        core.max_idle_queue_len = core.max_waiting;
                    }
                }
            )));

        idle_dropper
    }
}

/// Object that marks an input stream as no longer waiting for idle when it's dropped
pub (crate) struct IdleInputStreamCore(Option<Box<dyn Send + FnOnce() -> ()>>);

impl<'a> Drop for IdleInputStreamCore {
    fn drop(&mut self) {
        if let Some(on_drop) = self.0.take() {
            on_drop();
        }
    }
}

///
/// Sets the last message source for the subprogram owning an input stream
///
fn set_last_message_source<TMessage>(input_core: &Arc<Mutex<InputStreamCore<TMessage>>>, source_id: Option<SubProgramId>) {
    // Fetch the scene core and owner ID from the input core
    let (scene_core, owner_id) = {
        let input_core = input_core.lock().unwrap();
        (input_core.scene_core.clone(), input_core.program_id)
    };

    // Try to fetch the subprogram core from the scene core
    let subprogram_core = if let Some(scene_core) = scene_core.upgrade() {
        scene_core.lock().unwrap().get_sub_program(owner_id)
    } else {
        None
    };

    if let Some(subprogram_core) = subprogram_core {
        subprogram_core.lock().unwrap().last_message_source = source_id;
    }
}

impl<TMessage> Stream for InputStream<TMessage> {
    type Item=TMessage;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        use std::mem;

        set_last_message_source(&self.core, None);

        let mut core = self.core.lock().unwrap();

        if let Some((source, message)) = core.waiting_messages.pop_front() {
            // If any of the output sinks are waiting to write a value, wake them up as the queue has reduced
            let next_available = core.when_slots_available.pop_front();

            // The core is no longer idle
            core.idle = false;

            // Release the core lock before waking anything
            mem::drop(core);

            next_available.into_iter().for_each(|waker| waker.wake());

            // Set the last message source in the core
            set_last_message_source(&self.core, Some(source));

            // Return the message
            Poll::Ready(Some(message))
        } else if core.closed {
            // Once all the messages are delivered and the core is closed, close the stream
            Poll::Ready(None)
        } else {
            // Wait for the next message to be delivered
            core.when_message_sent = Some(cx.waker().clone());

            // The core has become idle
            core.idle = true;

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

        set_last_message_source(&self.core, None);

        let mut core = self.core.lock().unwrap();

        if let Some((source, message)) = core.waiting_messages.pop_front() {
            // If any of the output sinks are waiting to write a value, wake them up as the queue has reduced
            let next_available = core.when_slots_available.pop_front();

            // The core is no longer idle
            core.idle = false;

            // Release the core lock before waking anything
            mem::drop(core);

            next_available.into_iter().for_each(|waker| waker.wake());

            // Set the last message source in the core
            set_last_message_source(&self.core, Some(source));

            // Return the message
            Poll::Ready(Some((source, message)))
        } else if core.closed {
            // Once all the messages are delivered and the core is closed, close the stream
            Poll::Ready(None)
        } else {
            // Wait for the next message to be delivered
            core.when_message_sent = Some(cx.waker().clone());

            // The core has become idle
            core.idle = true;

            // Don't go to sleep until everything that's waiting for a slot has been woken up
            let next_available = core.when_slots_available.drain(..).collect::<Vec<_>>();

            // Release the core lock before waking anything
            mem::drop(core);

            next_available.into_iter().for_each(|waker| waker.wake());

            Poll::Pending
        }
    }
}

impl<TMessage> Drop for InputStream<TMessage> {
    fn drop(&mut self) {
        if self.active {
            let mut core = self.core.lock().unwrap();

            // Core becomes idle if the input stream is dropped (it will never process any messages again)
            core.idle   = true;

            // Stream is closed at this point, shouldn't handle any more messages
            core.closed = true;
        }
    }
}

impl<TMessage> Drop for InputStreamWithSources<TMessage> {
    fn drop(&mut self) {
        let mut core = self.core.lock().unwrap();

        // Core becomes idle if the input stream is dropped (it will never process any messages again)
        core.idle   = true;

        // Stream is closed at this point, shouldn't handle any more messages
        core.closed = true;
    }
}

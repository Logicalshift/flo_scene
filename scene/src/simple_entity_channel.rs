use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::immediate_entity_channel::*;

use ::desync::scheduler::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task;
use futures::task::{Context, Poll};

use std::mem;
use std::pin::*;
use std::sync::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::{VecDeque};

lazy_static! {
    static ref NEXT_TICKET_ID: AtomicUsize = AtomicUsize::new(0); 
}

///
/// A ticket ID is used to ensure that 
///
#[derive(Clone, Copy, PartialEq, Eq)]
struct TicketId(pub usize);

impl TicketId {
    ///
    /// Returns a unique ti ket for a pending request
    ///
    pub fn new() -> TicketId {
        let next_id = NEXT_TICKET_ID.fetch_add(1, Ordering::Relaxed);

        TicketId(next_id)
    }
}

///
/// A ticket represents a message that's waiting to be sent
///
struct Ticket<TMessage> {
    /// The ID of this ticket
    id: TicketId,

    /// Waker for the future that's waiting for this ticket to be processed
    waker: Option<task::Waker>,

    /// The message that will be sent by this ticket
    message: Option<TMessage>,
}

///
/// Shared state for the simple entity channel
///
struct SimpleEntityChannelCore<TMessage> {
    /// The maximum number of messages that can be ready at once
    buf_size: usize,

    /// The count of the number of channels that can talk to this core (the receiver gets closed if the number of channels goes to 0)
    num_channels: usize,

    /// The queue of messages ready for sending to the receiver
    ready_messages: VecDeque<TMessage>,

    /// Messages where the sending has been blocked
    waiting_tickets: VecDeque<Ticket<TMessage>>,

    /// Set to true if the receiver has been dropped or the channel has been closed some other way
    closed: bool,

    /// Immediate-mode messages that are waiting to be sent (None if this has not been created yet)
    immediate_queue: Option<Arc<JobQueue>>,

    /// The waker for the receiver for the core
    receiver_waker: Option<task::Waker>,
}

///
/// Future that represents a message waiting for a simple entity channel
///
struct WaitingMessage<TMessage> {
    /// The ID of the ticket corresponding to this message
    ticket_id: TicketId,

    /// The core that contains the ticket
    core: Weak<Mutex<SimpleEntityChannelCore<TMessage>>>,

    /// Set to true once this message has been completed (the ticket has been sent)
    completed: bool,
}

///
/// Stream that receives messages from a simple entity channel
///
struct SimpleEntityChannelReceiver<TMessage> {
    core: Arc<Mutex<SimpleEntityChannelCore<TMessage>>>
}

impl<TMessage> SimpleEntityChannelCore<TMessage> 
where
    TMessage:   'static + Send,
{
    ///
    /// Creates a new simple entity channel core, set up to have 1 open channel
    ///
    fn new(buf_size: usize) -> SimpleEntityChannelCore<TMessage> {
        SimpleEntityChannelCore {
            buf_size:           buf_size,
            ready_messages:     VecDeque::new(),
            num_channels:       1,
            waiting_tickets:    VecDeque::new(),
            closed:             false,
            receiver_waker:     None,
            immediate_queue:    None,
        }
    }

    ///
    /// Sends a message to the core in immediate mode
    ///
    fn send_message_now(_entity_id: EntityId, arc_self: &Arc<Mutex<SimpleEntityChannelCore<TMessage>>>, message: TMessage) -> Result<(), EntityChannelError> {
        let mut waiting = None;
        let mut err     = None;

        // Prepare to send the message by talking to the core
        let waker = {
            let mut core = arc_self.lock().unwrap();

            // Stop if the core is closed
            if core.closed {
                // Error is 'No longer listening' if the core is closed
                err = Some(EntityChannelError::NoLongerListening);

                None
            } else if core.ready_messages.len() < core.buf_size && core.waiting_tickets.len() == 0 {
                // Add the message to the ready buffer if the core is already ready
                core.ready_messages.push_back(message);

                core.receiver_waker.take()
            } else {
                // Wait for a particular slot to free up before sending the message. We support an unlimited number of futures waiting for a slot, and will send messages in the order that they were originally requested
                let ticket_id       = TicketId::new();
                let ticket          = Ticket {
                    id:         ticket_id,
                    waker:      None,
                    message:    Some(message),
                };

                core.waiting_tickets.push_back(ticket);

                // Create a future for when there's space for this message
                let immediate_queue = core.immediate_queue.get_or_insert_with(|| scheduler().create_job_queue());
                let waiting_future  = WaitingMessage {
                    ticket_id:  ticket_id,
                    core:       Arc::downgrade(arc_self),
                    completed:  false,
                };

                // Queue as a future_desync (so we can wait for it synchronously later on)
                waiting = Some(scheduler().future_desync(&*immediate_queue, move || {
                    async move { 
                        waiting_future.await
                    }.boxed()
                }));

                core.receiver_waker.take()
            }
        };

        // Wake the receiver if needed
        if let Some(waker) = waker {
            waker.wake();
        }

        // Stop if there's an error
        if let Some(err) = err {
            return Err(err);
        }

        // If the queue is generating backpressure, then syncronously wait for it to be ready
        if let Some(waiting) = waiting {
            waiting.sync().unwrap()?
        }

        Ok(())
    }

    ///
    /// Sends a message to the core
    ///
    fn send_message(_entity_id: EntityId, arc_self: &Arc<Mutex<SimpleEntityChannelCore<TMessage>>>, message: TMessage) -> impl Future<Output=Result<(), EntityChannelError>> {
        let mut waiting = None;
        let mut err     = None;

        // Prepare to send the message by talking to the core
        let waker = {
            let mut core = arc_self.lock().unwrap();

            // Stop if the core is closed
            if core.closed {
                // Error is 'No longer listening' if the core is closed
                err = Some(EntityChannelError::NoLongerListening);

                None
            } else if core.ready_messages.len() < core.buf_size && core.waiting_tickets.len() == 0 {
                // Add the message to the ready buffer if the core is already ready
                core.ready_messages.push_back(message);

                core.receiver_waker.take()
            } else {
                // Wait for a particular slot to free up before sending the message. We support an unlimited number of futures waiting for a slot, and will send messages in the order that they were originally requested
                let ticket_id   = TicketId::new();
                let ticket      = Ticket {
                    id:         ticket_id,
                    waker:      None,
                    message:    Some(message),
                };

                core.waiting_tickets.push_back(ticket);

                // Create a future for when there's space for this message
                waiting = Some(WaitingMessage {
                    ticket_id:  ticket_id,
                    core:       Arc::downgrade(arc_self),
                    completed:  false,
                });

                // Don't wake the receiver (it really should already be awake if the core is full anyway)
                None
            }
        };

        // Wake the receiver if needed
        if let Some(waker) = waker {
            waker.wake();
        }

        // Wait for the message to send, if there is one
        async move {
            // Stop immediately if there's an error
            if let Some(err) = err {
                return Err(err);
            }

            // If the buffer is full, wait for the ticket to come up
            if let Some(waiting) = waiting {
                waiting.await?;
            }

            Ok(())
        }
    }
}

impl<TMessage> Future for WaitingMessage<TMessage> {
    type Output = Result<(), EntityChannelError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Result<(), EntityChannelError>> {
        let ticket_id = self.ticket_id;

        // Acquire the core, assuming it still exists
        let core = if let Some(core) = self.core.upgrade() { core } else { return Poll::Ready(Err(EntityChannelError::NoLongerListening)); };

        // See if this ticket is ready to be sent, and return early if it's not
        let (receiver_waker, next_ticket_waker) = {
            let mut core = core.lock().unwrap();

            // Stop early if the core exists but has closed
            if core.closed {
                return Poll::Ready(Err(EntityChannelError::NoLongerListening));
            }

            if core.ready_messages.len() >= core.buf_size || core.waiting_tickets.front().map(|first| first.id) != Some(ticket_id) {
                // There's no space in the buffer, or this is not the first ticket
                for ticket in core.waiting_tickets.iter_mut() {
                    if ticket.id == ticket_id {
                        // Store the waker so this ticket can be woken up when there's space again
                        ticket.waker = Some(context.waker().clone());
                    }
                }

                // Mark as pending
                return Poll::Pending;
            }

            // There's space in the buffer, and we're the first ticket
            let mut our_ticket  = core.waiting_tickets.pop_front().unwrap();
            self.completed      = true;

            // Push onto the ready list
            if let Some(message) = our_ticket.message.take() {
                core.ready_messages.push_back(message);
            }

            // Need to wake the receiver and the next ticket
            let receiver_waker      = core.receiver_waker.take();
            let next_ticket_waker   = core.waiting_tickets.front_mut().and_then(|next_ticket| next_ticket.waker.take());

            (receiver_waker, next_ticket_waker)
        };

        // This ticket was ready to send: wake the main thread
        if let Some(receiver_waker) = receiver_waker {
            receiver_waker.wake();
        }

        // Also wake the next ticket, in case there's more space
        if let Some(next_ticket_waker) = next_ticket_waker {
            next_ticket_waker.wake();
        }

        Poll::Ready(Ok(()))
    }
}

impl<TMessage> Drop for WaitingMessage<TMessage> {
    fn drop(&mut self) {
        if let Some(core) = self.core.upgrade() {
            let next_ticket_waker = {
                let mut core = core.lock().unwrap();

                // Remove this ticket from the core, if the message is not sent
                if !self.completed {
                    core.waiting_tickets.retain(|ticket| ticket.id != self.ticket_id);
                }

                // Wake the first ticket to avoid a potential race where this was awoken just before it dropped
                core.waiting_tickets.front_mut().and_then(|next_ticket| next_ticket.waker.take())
            };

            if let Some(waker) = next_ticket_waker {
                waker.wake();
            }
        }
    }
}

impl<TMessage> Stream for SimpleEntityChannelReceiver<TMessage> {
    type Item = TMessage;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        // Try to receive the next message (and a waker for the first ticket)
        let (next_message, ticket_waker) = {
            let mut core = self.core.lock().unwrap();

            // Try to retrieve a message
            if let Some(message) = core.ready_messages.pop_front() {
                // We've got a message to send
                (Some(message), core.waiting_tickets.front_mut().and_then(|ticket| ticket.waker.take()))
            } else if core.closed {
                // Return 'closed' as soon as the ready messages are empty
                return Poll::Ready(None);
            } else {
                // The core is empty (need to wake the first ticket to send its message if it's not awake at the moment)
                core.receiver_waker = Some(context.waker().clone());

                (None, core.waiting_tickets.front_mut().and_then(|ticket| ticket.waker.take()))
            }
        };

        // Wake the ticket
        if let Some(ticket_waker) = ticket_waker {
            ticket_waker.wake();
        }

        // Return the message if there is one
        if let Some(next_message) = next_message {
            return Poll::Ready(Some(next_message));
        } else {
            return Poll::Pending;
        }
    }
}

impl<TMessage> Drop for SimpleEntityChannelReceiver<TMessage> {
    fn drop(&mut self) {
        let (wakers, old_tickets) = {
            let mut core = self.core.lock().unwrap();

            // Set the core as closed so no new messages can be added
            core.closed = true;

            // Clear the messages
            core.ready_messages = VecDeque::new();

            // Take the wakers for all of the tickets
            let wakers = core.waiting_tickets
                .iter_mut()
                .flat_map(|ticket| ticket.waker.take())
                .collect::<Vec<_>>();

            // Clear the tickets
            let mut old_tickets = VecDeque::new();
            mem::swap(&mut old_tickets, &mut core.waiting_tickets);

            (wakers, old_tickets)
        };

        // Wake all of the tickets so they can return errors (now the core is closed)
        wakers.into_iter()
            .for_each(|waker| waker.wake());

        // Drop the old tickets outside of the lock
        mem::drop(old_tickets);
    }
}

///
/// A simple entity channel just relays messages directly to a target channel
///
/// This provides an additional guarantee over what `mpsc::channel()` can provide for sending messages: at the point the future for
/// `send` or `send` is generated, the order that the message will be delivered in is fixed. This prevents race conditions
/// from forming where two messages can be delivered in a different order than expected.
///
pub struct SimpleEntityChannel<TMessage> {
    /// The core, used for sending messages
    core: Arc<Mutex<SimpleEntityChannelCore<TMessage>>>,

    /// The entity ID that owns this channel
    entity_id: EntityId,
}

impl<TMessage> SimpleEntityChannel<TMessage> 
where
    TMessage:   'static + Send,
{
    ///
    /// Creates a new entity channel
    ///
    pub fn new(entity_id: EntityId, buf_size: usize) -> (SimpleEntityChannel<TMessage>, impl 'static + Send + Stream<Item=TMessage>) {
        // Create the core
        let core = SimpleEntityChannelCore::new(buf_size);
        let core = Arc::new(Mutex::new(core));

        // Create the receiver
        let receiver = SimpleEntityChannelReceiver {
            core: Arc::clone(&core)
        };

        // Create the channel
        let channel = SimpleEntityChannel {
            core:       core,
            entity_id:  entity_id,
        };

        (channel, receiver)
    }

    ///
    /// Closes this channel
    ///
    pub fn close(&mut self) {
        let (receiver_waker, ticket_wakers) = {
            let mut core = self.core.lock().unwrap();

            // Set the core are closed
            core.closed = true;

            // Take the wakers for all of the tickets
            let wakers = core.waiting_tickets
                .iter_mut()
                .flat_map(|ticket| ticket.waker.take())
                .collect::<Vec<_>>();

            // Clear the tickets
            core.waiting_tickets = VecDeque::new();

            // Wake the receiver so it shuts down, and the tickets so they notice that the core is closed
            (core.receiver_waker.take(), wakers)
        };

        if let Some(receiver_waker) = receiver_waker {
            receiver_waker.wake();
        }

        ticket_wakers.into_iter().for_each(|ticket_waker| ticket_waker.wake());
    }
}

impl<TMessage> ImmediateEntityChannel for SimpleEntityChannel<TMessage> 
where
    TMessage:   'static + Send,
{
    #[inline]
    fn send_immediate(&mut self, message: Self::Message) -> Result<(), EntityChannelError> {
        // Send the message immediately
        SimpleEntityChannelCore::send_message_now(self.entity_id, &self.core, message)
    }
}

impl<TMessage> EntityChannel for SimpleEntityChannel<TMessage> 
where
    TMessage:   'static + Send,
{
    type Message    = TMessage;

    fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    fn is_closed(&self) -> bool {
        self.core.lock().unwrap().closed
    }

    fn send(&mut self, message: Self::Message) -> BoxFuture<'static, Result<(), EntityChannelError>> {
        // Send the message to the channel
        let future = SimpleEntityChannelCore::send_message(self.entity_id, &self.core, message);

        async move {
            future.await?;

            Ok(())
        }.boxed()
    }
}

impl<TMessage> Clone for SimpleEntityChannel<TMessage> {
    fn clone(&self) -> Self {
        // Add an extra channel to the core
        self.core.lock().unwrap().num_channels += 1;

        SimpleEntityChannel {
            core:       self.core.clone(),
            entity_id:  self.entity_id,
        }
    }
}

impl<TMessage> Drop for SimpleEntityChannel<TMessage> {
    fn drop(&mut self) {
        // Reduce the channel count
        let waker = {
            let mut core = self.core.lock().unwrap();

            // Reduce the number of channels and close the core if it reaches 0
            core.num_channels -= 1;

            if core.num_channels == 0 {
                core.closed = true;
                core.receiver_waker.take()
            } else {
                None
            }
        };

        // Wake the receiver if the number of channels is 0
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use futures::future;
    use futures::executor;

    #[test]
    fn receive_from_buffer() {
        let (channel, receiver) = SimpleEntityChannel::<()>::new(EntityId::new(), 10);

        // Fill with 5 pending requests (first request will be 'sent' straight away)
        let mut channel = channel;
        let requests    = (0..6).into_iter().map(|_| {
            let msg = channel.send(());
            async move {
                msg.await.unwrap();
            }.boxed()
        });
        let results     = async move {
            let mut receiver = receiver;
            for i in 0..6 {
                receiver.next().await.unwrap();
                println!("Received {}", i);
            }
        };

        let all_futures = vec![results.boxed()].into_iter().chain(requests).collect::<Vec<_>>();

        executor::block_on(async move {
            future::join_all(all_futures).await;
        });
    }

    #[test]
    fn overfill_then_drain() {
        let (channel, receiver) = SimpleEntityChannel::<()>::new(EntityId::new(), 1);

        // Fill with 5 pending requests (first request will be 'sent' straight away)
        let mut channel = channel;
        let requests    = (0..6).into_iter().map(|_| {
            let msg = channel.send(());
            async move {
                msg.await.unwrap();
            }.boxed()
        });
        let results     = async move {
            let mut receiver = receiver;
            for i in 0..6 {
                receiver.next().await.unwrap();
                println!("Received {}", i);
            }
        };

        let all_futures = vec![results.boxed()].into_iter().chain(requests).collect::<Vec<_>>();

        executor::block_on(async move {
            future::join_all(all_futures).await;
        });
    }

    #[test]
    fn overfilled_ordering() {
        let (channel, receiver) = SimpleEntityChannel::<usize>::new(EntityId::new(), 2);

        // Fill with 5 pending requests (first request will be 'sent' straight away)
        let mut channel = channel;
        let requests    = (0..10).into_iter().map(|i| {
            let msg = channel.send(i);
            async move {
                msg.await.unwrap();
            }.boxed()
        });
        let results     = async move {
            let mut receiver = receiver;
            for i in 0..10 {
                let msg = receiver.next().await.unwrap();
                println!("Received {} {}", i, msg);

                assert!(i == msg);
            }
        };

        let all_futures = vec![results.boxed()].into_iter().chain(requests).collect::<Vec<_>>();

        executor::block_on(async move {
            future::join_all(all_futures).await;
        });
    }
}

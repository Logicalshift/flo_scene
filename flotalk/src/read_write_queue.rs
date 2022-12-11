use futures::prelude::*;
use futures::future;
use futures::task;
use futures::task::{Waker, Poll};

use once_cell::sync::{Lazy};

use std::cell::*;
use std::collections::{VecDeque};
use std::ops::{Deref, DerefMut};
use std::sync::*;
use std::sync::atomic::{Ordering, AtomicUsize};

static NEXT_TICKET_ID: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

///
/// A ticket is a unique identifier for a lock that's waiting to be created
///
#[derive(Copy, Clone, PartialEq, Eq)]
struct Ticket(usize);

///
/// Releases the specified ticket when dropped, if the ticket has not been claimed
///
struct TicketHolder<'a, TData>(&'a ReadWriteQueue<TData>, Option<Ticket>);

///
/// A waiting lock 
///
enum WaitingLock {
    /// A number of futures are waiting for a read lock in parallel
    ReadLock(Vec<(Ticket, Option<Waker>)>),

    /// A future is waiting for an exclusive write lock
    WriteLock(Ticket, Option<Waker>),
}

///
/// Represents the locks and waiting locks against a read-write queue
///
struct Locks {
    /// The lock that is currently held
    held: WaitingLock,

    /// The locks that are waiting to be taken
    waiting: VecDeque<WaitingLock>,
}

///
/// Provides access to a piece of data on a per-future basis
///
pub struct ReadWriteQueue<TData> {
    /// The data protected by this queue
    data: UnsafeCell<TData>,

    /// The held and waiting locks for this object
    locks: Mutex<Option<Box<Locks>>>,
}

///
/// A read-only lock taken against a read-write queue
///
pub struct ReadOnlyData<'a, TData> {
    owner:  &'a ReadWriteQueue<TData>,
    ticket: Ticket,
    data:   &'a TData,
}

///
/// A writeable lock taken against a read-write queue
///
pub struct WriteableData<'a, TData> {
    owner:  &'a ReadWriteQueue<TData>,
    ticket: Ticket,
    data:   &'a mut TData,
}

impl<TData> ReadWriteQueue<TData> {
    ///
    /// Creates a new read-write queue
    ///
    pub fn new(data: TData) -> Self {
        ReadWriteQueue {
            data:   UnsafeCell::new(data),
            locks:  Mutex::new(None),
        }
    }

    ///
    /// Indicates that a lock is done with by releasing the ticket. The lock must be held for this call to be valid
    ///
    fn release(&self, release_ticket: Ticket) {
        let to_notify = {
            let mut maybe_locks = self.locks.lock().unwrap();

            if let Some(locks) = &mut *maybe_locks {
                // Remove the ticket from the held list
                match &mut locks.held {
                    WaitingLock::WriteLock(held_ticket, _) => {
                        debug_assert!(release_ticket == *held_ticket);

                        if release_ticket == *held_ticket {
                            // Fetch the next lock
                            if let Some(next_lock) = locks.waiting.pop_front() {
                                // Set this as the current held lock
                                locks.held = next_lock;

                                // Wake up anything waiting on this lock
                                locks.held.take_wakers()
                            } else {
                                // Nothing is waiting for a lock: nothing to wake
                                *maybe_locks = None;
                                vec![]
                            }
                        } else {
                            // Not valid to call release() without a held ticket
                            unreachable!()
                        }
                    }

                    WaitingLock::ReadLock(tickets) => {
                        if let Some(ticket_pos) = tickets.iter().position(|(held_ticket, _waker)| *held_ticket == release_ticket) {
                            // Remove from the set of read tickets
                            tickets.remove(ticket_pos);

                            if tickets.is_empty() {
                                // All locks released
                                if let Some(next_lock) = locks.waiting.pop_front() {
                                    // Set this as the current held lock
                                    locks.held = next_lock;

                                    // Wake up anything waiting on this lock
                                    locks.held.take_wakers()
                                } else {
                                    // Nothing is waiting for a lock: nothing to wake
                                    *maybe_locks = None;
                                    vec![]
                                }
                            } else {
                                // Other read locks are still held, need to wait for them all to release
                                vec![]
                            }

                        } else {
                            // Not valid to call release() without a held ticket
                            unreachable!("Read ticket was not held")
                        }
                    }
                }
            } else {
                // No locks are held
                unreachable!()
            }
        };

        // Wake the notifiers
        to_notify.into_iter().for_each(|waker| waker.wake());
    }

    ///
    /// If this is in a state where it can be read from, return the readable data immediately
    ///
    pub fn try_read<'a>(&'a self) -> Option<ReadOnlyData<'a, TData>> {
        let ticket = Ticket::new();

        // Try to lock immediately, or generate a ticket
        let mut maybe_locks = self.locks.lock().unwrap();
        if let Some(locks) = &mut *maybe_locks {
            // Lock is held at least once (we can acquire it immediately if nothing is waiting and it's held as a read lock)
            match &mut locks.held {
                WaitingLock::ReadLock(owners) => {
                    if locks.waiting.len() == 0 {
                        // There's one lock owner, and it's a read lock, so we can read in parallel with it
                        owners.push((ticket, None));

                        // Safe to return the data
                        unsafe { return Some(ReadOnlyData { owner: self, ticket: ticket, data: &*self.data.get() }) };
                    }
                }

                WaitingLock::WriteLock(_, _) => { /* Always need to wait for a write lock to be released */ }
            }
        } else {
            // Lock is not held: create a read lock
            *maybe_locks = Some(Box::new(Locks {
                held:       WaitingLock::ReadLock(vec![(ticket, None)]),
                waiting:    VecDeque::new()
            }));

            // Safe to return the data
            unsafe { return Some(ReadOnlyData { owner: self, ticket: ticket, data: &*self.data.get() }) };
        }

        None
    }

    ///
    /// If this is in a state where it can be read from, return the readable data immediately
    ///
    #[inline]
    pub fn try_write<'a>(&'a self) -> Option<WriteableData<'a, TData>> {
        let ticket = Ticket::new();

        // Try to lock immediately, or generate a ticket
        let mut maybe_locks = self.locks.lock().unwrap();
        if let Some(_) = &mut *maybe_locks {
            // Lock is held by something
        } else {
            // Lock is not held: create a write lock
            *maybe_locks = Some(Box::new(Locks {
                held:       WaitingLock::WriteLock(ticket, None),
                waiting:    VecDeque::new()
            }));

            // Safe to return the data
            unsafe { return Some(WriteableData { owner: self, ticket: ticket, data: &mut *self.data.get() }) };
        }

        None
    }


    ///
    /// Returns a future that has read acess to the data protected by this queue
    ///
    /// Access is granted in the same order that it's requested. In particular, even if a read lock is already held, this will wait if
    /// a write lock has also been requested.
    ///
    pub fn read<'a>(&'a self) -> impl 'a + Future<Output=ReadOnlyData<'a, TData>> {
        async move {
            // Create a ticket for this read lock
            let ticket = Ticket::new();

            {
                // Try to lock immediately, or generate a ticket
                let mut maybe_locks = self.locks.lock().unwrap();
                if let Some(locks) = &mut *maybe_locks {
                    // Lock is held at least once (we can acquire it immediately if nothing is waiting and it's held as a read lock)
                    match &mut locks.held {
                        WaitingLock::ReadLock(owners) => {
                            if locks.waiting.len() == 0 {
                                // There's one lock owner, and it's a read lock, so we can read in parallel with it
                                owners.push((ticket, None));

                                // Safe to return the data
                                unsafe { return ReadOnlyData { owner: self, ticket: ticket, data: &*self.data.get() } };
                            }
                        }

                        WaitingLock::WriteLock(_, _) => { /* Always need to wait for a write lock to be released */ }
                    }
                } else {
                    // Lock is not held: create a read lock
                    *maybe_locks = Some(Box::new(Locks {
                        held:       WaitingLock::ReadLock(vec![(ticket, None)]),
                        waiting:    VecDeque::new()
                    }));

                    // Safe to return the data
                    unsafe { return ReadOnlyData { owner: self, ticket: ticket, data: &*self.data.get() } };
                }

                // There's at least one held lock, and we need to queue at the end
                if let Some(locks) = &mut *maybe_locks {
                    match locks.waiting.back_mut() {
                        Some(WaitingLock::ReadLock(waiting)) => {
                            // Take this read-only lock alongside an existing set of read-only locks
                            waiting.push((ticket, None));
                        }

                        _ => {
                            // Lock after the current set of locks
                            locks.waiting.push_back(WaitingLock::ReadLock(vec![(ticket, None)]));
                        }
                    }
                } else {
                    // If locks were 'None' then we would have taken the read lock by now
                    unreachable!()
                }
            }

            // Wait for the lock to become available
            let _holder = TicketHolder(self, Some(ticket));

            future::poll_fn(|context| {
                let mut maybe_locks = self.locks.lock().unwrap();

                if let Some(locks) = &mut *maybe_locks {
                    // Ready if the lock is already held
                    if locks.held.is_for_ticket(ticket) {
                        return Poll::Ready(());
                    }

                    // Set this context to wake up the lock
                    locks.waiting.iter_mut().for_each(|lock| lock.wake_in_context(ticket, context));
                } else {
                    // The locks can't go to the 'no locks held' state while we're waiting to obtain a lock
                    unreachable!()
                }

                Poll::Pending
            }).await;

            // Lock acquired: safe to return the data
            _holder.claim();
            unsafe { return ReadOnlyData { owner: self, ticket: ticket, data: &*self.data.get() } };
        }
    }

    ///
    /// Returns a future that has write access to the data protected by this queue
    ///
    /// Access is granted in the same order that it's requested.
    ///
    pub fn write<'a>(&'a self) -> impl 'a + Future<Output=WriteableData<'a, TData>> {
        async move {
            // Create a ticket for this write lock
            let ticket = Ticket::new();

            {
                // Try to lock immediately, or generate a ticket
                let mut maybe_locks = self.locks.lock().unwrap();
                if let Some(_) = &mut *maybe_locks {
                    // Lock is held at least once (we always wait for a write lock)
                } else {
                    // Lock is not held: create a write lock
                    *maybe_locks = Some(Box::new(Locks {
                        held:       WaitingLock::WriteLock(ticket, None),
                        waiting:    VecDeque::new()
                    }));

                    // Safe to return the data
                    unsafe { return WriteableData { owner: self, ticket: ticket, data: &mut *self.data.get() } };
                }

                // There's at least one held lock, and we need to queue at the end
                if let Some(locks) = &mut *maybe_locks {
                    // Lock after the current set of locks
                    locks.waiting.push_back(WaitingLock::WriteLock(ticket, None));
                } else {
                    // If locks were 'None' then we would have taken the write lock by now
                    unreachable!()
                }
            }

            // Wait for the lock to become available
            let _holder = TicketHolder(self, Some(ticket));

            future::poll_fn(|context| {
                let mut maybe_locks = self.locks.lock().unwrap();

                if let Some(locks) = &mut *maybe_locks {
                    // Ready if the lock is already held
                    if locks.held.is_for_ticket(ticket) {
                        return Poll::Ready(());
                    }

                    // Set this context to wake up the lock
                    locks.waiting.iter_mut().for_each(|lock| lock.wake_in_context(ticket, context));
                } else {
                    // The locks can't go to the 'no locks held' state while we're waiting to obtain a lock
                    unreachable!()
                }

                Poll::Pending
            }).await;

            // Lock acquired: safe to return the data
            _holder.claim();
            unsafe { return WriteableData { owner: self, ticket: ticket, data: &mut *self.data.get() } };
        }
    }
}

impl Ticket {
    ///
    /// Returns a unique ticket for a pending request
    ///
    #[inline]
    pub fn new() -> Ticket {
        let next_id = NEXT_TICKET_ID.fetch_add(1, Ordering::Relaxed);

        Ticket(next_id)
    }
}

impl<'a, TData> ReadOnlyData<'a, TData> {
    ///
    /// Upgrades this read-only lock to a writeable lock
    ///
    pub fn upgrade(self) -> impl 'a + Future<Output=WriteableData<'a, TData>> {
        self.owner.write()
    }
}

impl<'a, TData> Deref for ReadOnlyData<'a, TData> {
    type Target = TData;

    #[inline]
    fn deref(&self) -> &TData {
        self.data
    }
}

impl<'a, TData> Drop for ReadOnlyData<'a, TData> {
    #[inline]
    fn drop(&mut self) {
        self.owner.release(self.ticket);
    }
}

impl<'a, TData> Deref for WriteableData<'a, TData> {
    type Target = TData;

    #[inline]
    fn deref(&self) -> &TData {
        self.data
    }
}

impl<'a, TData> DerefMut for WriteableData<'a, TData> {
    #[inline]
    fn deref_mut(&mut self) -> &mut TData {
        self.data
    }
}

impl<'a, TData> Drop for WriteableData<'a, TData> {
    #[inline]
    fn drop(&mut self) {
        self.owner.release(self.ticket);
    }
}

impl<'a, TData> TicketHolder<'a, TData> {
    #[inline]
    fn claim(mut self) {
        self.1 = None;
    }
}

impl<'a, TData> Drop for TicketHolder<'a, TData> {
    #[inline]
    fn drop(&mut self) {
        if let Some(ticket) = self.1.take() {
            self.0.release(ticket);
        }
    }
}

impl WaitingLock {
    ///
    /// Removes and returns the wakers for this ticket
    ///
    fn take_wakers(&mut self) -> Vec<Waker> {
        match self {
            WaitingLock::ReadLock(tickets)      => tickets.iter_mut().flat_map(|(_, waker)| waker.take()).collect(),
            WaitingLock::WriteLock(_, waker)    => waker.take().into_iter().collect(),
        }
    }

    ///
    /// True if this lock is for the specified ticket
    ///
    fn is_for_ticket(&self, ticket: Ticket) -> bool {
        match self {
            WaitingLock::ReadLock(tickets)          => tickets.iter().any(|(our_ticket, _)| *our_ticket == ticket),
            WaitingLock::WriteLock(our_ticket, _)   => *our_ticket == ticket,
        }
    }

    ///
    /// Sets the waker for the specified ticket to the waker for a task context
    ///
    fn wake_in_context(&mut self, ticket: Ticket, context: &task::Context) {
        match self {
            WaitingLock::ReadLock(tickets)              => tickets.iter_mut().for_each(|(our_ticket, waker)| { if *our_ticket == ticket { *waker = Some(context.waker().clone()); } }),
            WaitingLock::WriteLock(our_ticket, waker)   => { if *our_ticket == ticket { *waker = Some(context.waker().clone()); } },
        }
    }
}

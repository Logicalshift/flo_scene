use super::context::*;
use super::continuation::*;
use super::error::*;
use super::initialization::*;
use super::message::*;
use super::reference::*;
use super::releasable::*;
use super::standard_classes::*;
use super::symbol::*;
use super::symbol_table::*;
use super::value::*;

use once_cell::sync::{Lazy};
use futures::prelude::*;
use futures::future;
use futures::lock;
use futures::task::{Poll, Context, ArcWake};

use flo_stream::*;

use std::collections::{HashSet, HashMap};
use std::sync::*;

// TODO: write and upgrade to a 'fair' mutex that processing wakeups in the order that they happen

///
/// Waker that sets a particular future to be woken before waking up the main future
///
struct BackgroundFutureWaker {
    /// The ID of the future to mark as awake
    future_id: usize,

    /// The background tasks structure where the main waker and woken futures are stored
    background_tasks: Arc<Mutex<TalkContextBackgroundTasks>>,
}

///
/// An owner for `TalkValues` that belong to a runtime
///
/// Runtime values like this are released the next time a runtime is asked to perform an action
///
#[derive(Clone)]
pub struct TalkOwnedByRuntime {
    /// Values that are waiting for release from the runtime
    released_values: Arc<Mutex<Vec<TalkValue>>>,
}

///
/// A `TalkRuntime` is used to run continuations inside a `TalkContext` (it wraps a TalkContext,
/// and schedules continuations on them)
///
pub struct TalkRuntime {
    /// The context that this runtime is managing
    pub (crate) context: Arc<lock::Mutex<TalkContext>>,

    /// Values generated previously by the runtime that are waiting to be released
    waiting_for_release: Arc<Mutex<Vec<TalkValue>>>,
}

impl TalkRuntime {
    ///
    /// Creates a runtime for a context
    ///
    pub fn with_context(context: TalkContext) -> TalkRuntime {
        TalkRuntime {
            context:                Arc::new(lock::Mutex::new(context)),
            waiting_for_release:    Arc::new(Mutex::new(vec![])),
        }
    }

    ///
    /// Returns an empty runtime
    ///
    pub fn empty() -> TalkRuntime {
        Self::with_context(TalkContext::empty())
    }

    ///
    /// Creates a new runtime with the standard set of symbols declared
    ///
    pub fn with_standard_symbols() -> impl Future<Output=TalkRuntime> {
        async {
            let runtime = Self::empty();
            runtime.run(talk_init_standard_classes()).await;

            runtime
        }
    }

    ///
    /// Sends a message to a value using this runtime
    ///
    pub fn send_message<'a>(&'a self, value: &'a TalkValue, message: TalkMessage) -> impl 'a + Send + Future<Output=TalkOwned<TalkValue, TalkOwnedByRuntime>> {
        async move {
            self.run(TalkContinuation::Soon(Box::new(move |talk_context| {
                let value = value.clone_in_context(talk_context);
                value.send_message_in_context(message, talk_context)
            }))).await
        }
    }

    ///
    /// Releases a TalkValue
    ///
    /// FloTalk uses a reference-counting system for values; failing to call release_value() on a TalkValue will leak it
    ///
    pub fn release_value<'a>(&'a self, value: TalkValue) -> impl 'a + Send + Future<Output=()> {
        async move {
            self.run(TalkContinuation::soon(move |talk_context| {
                value.release(talk_context);
                TalkValue::Nil.into()
            })).await;
        }
    }

    ///
    /// Runs a continuation with a 'later' part
    ///
    fn run_continuation_later<'a>(talk_context: Weak<lock::Mutex<TalkContext>>, later: Box<dyn 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkContinuation<'static>>>) -> impl 'a + Send + Future<Output=TalkContinuation<'static>> {
        // If the runtime is dropped while the future is running, it will be aborted (if it ever wakes up again)
        let mut acquire_context = None;
        let mut later           = later;

        // Poll the 'later' whenever the context is available
        future::poll_fn(move |future_context| {
            if let Some(talk_context) = talk_context.upgrade() {
                // Often we can just acquire the mutex immediately
                if acquire_context.is_none() {
                    // Don't try_lock() if we're acquiring the context via the mutex
                    if let Some(mut talk_context) = talk_context.try_lock() {
                        acquire_context = None;
                        return later(&mut *talk_context, future_context);
                    }
                }

                // Start locking the context if it's currently released
                if acquire_context.is_none() {
                    acquire_context = Some(lock::Mutex::lock_owned(talk_context));
                }

                if let Poll::Ready(mut talk_context) = acquire_context.as_mut().unwrap().poll_unpin(future_context) {
                    // Acquired access to the context
                    acquire_context = None;

                    return later(&mut *talk_context, future_context);
                } else {
                    // Context is in use on another thread
                    return Poll::Pending;
                }
            } else {
                // Context is not available
                acquire_context = None;

                Poll::Ready(TalkError::RuntimeDropped.into())
            }
        })
    }

    ///
    /// Replaces the existing root symbol table with a new empty one
    ///
    pub async fn create_empty_root_symbol_table(&self) {
        self.run(TalkContinuation::soon(|talk_context| {
            talk_context.create_empty_root_symbol_table();
            ().into()
        })).await;
    }

    ///
    /// Sets the value of a symbol in the root symbol table (defining it if necessary)
    ///
    pub async fn set_root_symbol_value<'a>(&self, symbol: impl Send + Into<TalkSymbol>, new_value: impl Send + Into<TalkValue>) {
        self.run(TalkContinuation::soon(move |talk_context| {
            talk_context.set_root_symbol_value(symbol, new_value.into());
            ().into()
        })).await;
    }

    ///
    /// Evaluates a continuation, then sends a stream of messages to the resulting value.
    ///
    /// The future will return once all of the messages in the stream have been consumed. The stream will not be consumed if the original continuation produces 
    /// an error. If any of the messages generate an error, the rest of the stream will be discarded. The return value is the value that the messages were sent
    /// to.
    ///
    pub fn stream_to<'a, TStream>(&'a self, create_receiver: impl Into<TalkContinuation<'a>>, stream: TStream) -> impl 'a + Send + Future<Output=Result<TalkOwned<TalkValue, TalkOwnedByRuntime>, TalkError>> 
    where
        TStream:        'a + Send + Stream,
        TStream::Item:  Send + TalkMessageType,
    {
        let create_receiver = create_receiver.into();

        async move {
            let mut stream = Box::pin(stream);

            // Fetch the value representing the targets of the messages
            let target = self.run(create_receiver).await;

            // Stop if the target produces an error
            if let TalkValue::Error(error) = &*target {
                let error = error.clone();
                return Err(error);
            }

            // Read from the stream and send to the target
            while let Some(msg) = stream.next().await {
                // Create a continuation to send the message
                let send_message = {
                    let context         = self.context.lock().await;
                    let msg             = msg.to_message(&*context);
                    let continuation    = target.clone_in_context(&*context).send_message_in_context(msg.leak(), &*context);
                    continuation
                };

                // Send to the script
                let result = self.run(send_message).await;

                // Stop early if there was an error
                if let TalkValue::Error(error) = &*result {
                    let error = error.clone();
                    return Err(error);
                }
            }

            Ok(target)
        }
    }

    ///
    /// Evaluates a continuation, then sends the message `value: output` to the result, where 'output' is an object that sends all its message to the
    /// returned stream. Opposite of `stream_to`.
    ///
    /// This seems complicated, but really is pretty simple to use in practice - just use  a block with a parameter:
    ///
    /// ```no_run
    /// # #[macro_use] extern crate flo_talk_macros;
    /// # use flo_talk::*;
    /// # let runtime = TalkRuntime::empty();
    /// #[derive(TalkMessageType)]
    /// enum HelloWorld { #[message("helloWorld")] Hello, #[message("goodbye")] Goodbye }
    ///
    /// let mut hello_world = runtime.stream_from::<HelloWorld>(TalkScript::from("[ :output | output helloWorld. output goodbye. ]"));
    /// ```
    ///
    pub fn stream_from<TStreamItem>(&self, receive_target: impl Into<TalkContinuation<'static>>) -> impl 'static + Send + Stream<Item=Result<TStreamItem, TalkError>> + TryStream<Ok=TStreamItem, Error=TalkError>
    where
        TStreamItem: 'static + Send + TalkMessageType,
    {
        use futures::future::{Either};
        static VALUE_COLON_MSG: Lazy<TalkSymbol>  = Lazy::new(|| "value:".into());

        let context             = Arc::clone(&self.context);
        let waiting_for_release = Arc::clone(&self.waiting_for_release);
        let receive_target      = receive_target.into();

        generator_stream(move |yield_value| {
            async move {
                // Create the sender object and the receiver stream
                let (sender, receiver)  = {
                    let mut context         = context.lock().await;
                    let (sender, receiver)  = create_talk_sender::<TStreamItem>(&mut *context);
                    (sender.leak(), receiver)
                };

                // Evaluate the value that we'll send the sender object to
                let receive_target = Self::run_with_context(receive_target, context.clone(), waiting_for_release.clone()).await;
                if let TalkValue::Error(err) = &*receive_target {
                    // Stop early if the target is an error
                    sender.release_in_context(&*context.lock().await);
                    yield_value(Err(err.clone())).await;
                    return;
                }

                // Start sending the 'value:' message (this runs in parallel with our relay code)
                let send_message = receive_target.leak().send_message_in_context(TalkMessage::with_arguments(vec![(*VALUE_COLON_MSG, sender)]), &*context.lock().await);
                let send_message = Self::run_with_context(send_message, context.clone(), waiting_for_release.clone());

                // Create a future to relay results from the output to the stream
                let relay_message = async move {
                    let mut receiver = receiver;

                    while let Some(item) = receiver.next().await {
                        yield_value(Ok(item)).await;
                    }
                };

                // Run until both futures finish, or abort early if send_message errors out
                match future::select(Box::pin(send_message), Box::pin(relay_message)).await {
                    Either::Left((send_message_result, relay_message)) => {
                        // The send_message call returned before the relay finished
                        if let TalkValue::Error(err) = &*send_message_result {
                            // Abort early and report the error
                            // yield_value(Err(err.clone())); -- TODO
                            return;
                        } else {
                            // Otherwise, release the result and wait for the relay to finish
                            relay_message.await;
                        }
                    }

                    Either::Right((_, send_message)) => {
                        // The relay finished but the send_message call is still going. Close the stream once the call finishes
                        let send_message_result = send_message.await;

                        if let TalkValue::Error(err) = &*send_message_result {
                            // In spite of the stream being finished at this point, send_message errored anyway, so we'll report that
                            // yield_value(Err(err.clone())); -- TODO
                            return;
                        }
                    }
                }
            }
        })
    }

    ///
    /// Runs a continuation using the values stored in a runtime (the continuation and the future can outlive the runtime itself if necessary)
    ///
    fn run_with_context<'a>(continuation: impl Into<TalkContinuation<'a>>, context: Arc<lock::Mutex<TalkContext>>, waiting_for_release: Arc<Mutex<Vec<TalkValue>>>) -> impl 'a + Send + Future<Output=TalkOwned<TalkValue, TalkOwnedByRuntime>> {
        // Start running the continuation
        let mut continuation = continuation.into();

        // Take this opportunity to obtain the context and release any values that were previously returned
        use std::mem;

        let mut released_values = vec![];
        mem::swap(&mut *waiting_for_release.lock().unwrap(), &mut released_values);

        // The owner points at the waiting_for_release list so the next 'run' call can release the values created by this one
        let owner = TalkOwnedByRuntime { released_values: waiting_for_release };

        async move {
            // Release anything that is waiting before starting the new continuation
            if !released_values.is_empty() {
                let context = context.lock().await;
                released_values.into_iter().for_each(|val| val.release_in_context(&*context));
            }

            let talk_context = Arc::downgrade(&context);
            mem::drop(context);

            // Either return the value stored in the continuation or await the continuation
            loop {
                // Each step either creates a new continuation or returns the result
                continuation = match continuation {
                    TalkContinuation::Ready(value)  => { return TalkOwned::new(value, owner) }

                    TalkContinuation::Soon(soon)    => { 
                        // Upgrade and lock the context
                        let talk_context        = if let Some(talk_context) = talk_context.upgrade() { talk_context } else { return TalkOwned::new(TalkError::RuntimeDropped.into(), owner); };
                        let mut talk_context    = if let Some(talk_context) = talk_context.try_lock() { talk_context } else { talk_context.lock().await };

                        // Update the continuation to the 'soon' continuation
                        soon(&mut *talk_context)
                    }

                    TalkContinuation::Later(later)  => {
                        // Poll the 'later' function with the context
                        let talk_context = talk_context.clone();
                        Self::run_continuation_later(talk_context, later).await
                    }

                    TalkContinuation::Future(later) => {
                        later.await
                    }
                }
            }
        }
    }

    ///
    /// Returns a future that will poll any background tasks contained by the context
    ///
    /// This can be run multiple times: only one running future will actually run the background tasks, and background tasks will progress provided at least
    /// one of these futures exist.
    ///
    pub fn run_background_tasks(&self) -> impl Send + Future<Output=()> {
        let weak_context        = Arc::downgrade(&self.context);
        let waiting_for_release = Arc::clone(&self.waiting_for_release);

        async move {
            // Setup: fetch the background tasks from the context
            let background_tasks = { 
                if let Some(talk_context) = weak_context.upgrade() {
                    // Take the background tasks from the context
                    talk_context.lock().await.background_tasks.clone()
                } else {
                    // Stop if the context is no longer available
                    return;
                }
            };

            // Poll the background tasks
            future::poll_fn(move |future_context| {
                // TODO: add a way to reawaken a thread if a different 'run_background_tasks' future is dropped

                // Update the background task waker to reawaken this thread and get the IDs of the 'awake' continuations we need to poll
                let mut awake_futures = {
                    use std::mem;
                    let mut background_tasks = background_tasks.lock().unwrap();

                    if background_tasks.context_dropped {
                        // Stop if the context is dropped
                        return Poll::Ready(());
                    }

                    // Convert any new continuations into futures
                    if !background_tasks.new_continuations.is_empty() {
                        let mut new_continuations = HashMap::new();
                        mem::swap(&mut new_continuations, &mut background_tasks.new_continuations);

                        for (id, continuation) in new_continuations {
                            // Start the continuation running as a future
                            let weak_context        = weak_context.clone();
                            let waiting_for_release = waiting_for_release.clone();
                            let run_continuation    = async move {
                                let mut continuation = continuation;

                                loop {
                                    continuation = match continuation {
                                        TalkContinuation::Ready(val)        => {
                                            // Lock the context
                                            let talk_context    = if let Some(talk_context) = weak_context.upgrade() { talk_context } else { break; };
                                            let talk_context    = talk_context.lock().await;

                                            // Release any value that was generated (background values are never used)
                                            val.release_in_context(&*talk_context);

                                            // Stop running
                                            break;
                                        }

                                        TalkContinuation::Soon(soon)        => {
                                            // Lock the context and run 'soon'
                                            let talk_context        = if let Some(talk_context) = weak_context.upgrade() { talk_context } else { break; };
                                            let mut talk_context    = talk_context.lock().await;

                                            soon(&mut *talk_context)
                                        }

                                        TalkContinuation::Later(later)      => Self::run_continuation_later(weak_context.clone(), later).await,
                                        TalkContinuation::Future(future)    => future.await,
                                    }
                                }
                            };

                            // Store as an awake running future so we poll it
                            background_tasks.running_futures.insert(id, Some(run_continuation.boxed()));
                            background_tasks.awake_futures.insert(id);
                        }
                    }

                    // Wake up this future next time something changes
                    background_tasks.waker = Some(future_context.waker().clone());

                    // Take the awake continuation IDs from the background tasks
                    let mut awake_futures = HashSet::new();
                    mem::swap(&mut awake_futures, &mut background_tasks.awake_futures);

                    awake_futures
                };

                // Poll any awake futures
                for id in awake_futures {
                    let maybe_future = background_tasks.lock().unwrap().running_futures.get_mut(&id).map(|future| future.take());

                    if let Some(maybe_future) = maybe_future {
                        if let Some(mut future) = maybe_future {
                            use futures::task;

                            // Create a waker for this future that marks it as 'awake' to use with our new future
                            let waker               = Arc::new(BackgroundFutureWaker { future_id: id, background_tasks: Arc::clone(&background_tasks) });
                            let future_waker        = task::waker(waker);
                            let mut future_context  = task::Context::from_waker(&future_waker);

                            if let Poll::Pending = future.poll_unpin(&mut future_context) {
                                // Future is still available to run, so add back again
                                background_tasks.lock().unwrap().running_futures.insert(id, Some(future));
                            } else {
                                // Future has finished
                                background_tasks.lock().unwrap().running_futures.remove(&id);
                            }
                        } else {
                            // TODO: Future has been re-awakened while being polled (has a slot with value 'None') - need to re-poll it!
                        }
                    } else {
                        // Future has finished after being awakened (nothing more to do)
                    }
                }

                // Keep on running background tasks while the context is alive
                Poll::Pending
            }).await
        }
    }

    ///
    /// Runs a continuation or a script using this runtime
    ///
    pub fn run<'a>(&self, continuation: impl 'a + Into<TalkContinuation<'a>>) -> impl 'a + Send + Future<Output=TalkOwned<TalkValue, TalkOwnedByRuntime>> {
        Self::run_with_context(continuation, self.context.clone(), self.waiting_for_release.clone())
    }

    ///
    /// Generates a symbol table, then runs a continuation with it
    ///
    pub fn run_with_symbols<'a>(&'a self, create_symbol_table: impl 'a + Send + FnOnce(&mut TalkContext) -> Vec<(TalkSymbol, TalkValue)>, create_continuation: impl 'a + Send + FnOnce(Arc<Mutex<TalkSymbolTable>>, Vec<TalkCellBlock>) -> TalkContinuation<'static>) -> impl 'a + Send + Future<Output=TalkOwned<TalkValue, TalkOwnedByRuntime>> {
        let continuation = TalkContinuation::Soon(Box::new(move |talk_context| {
            // Ask for the symbol table
            let symbols             = create_symbol_table(talk_context);

            // Create a cell block to contain the symbols
            let cell_block          = talk_context.allocate_cell_block(symbols.len());
            let mut symbol_table    = TalkSymbolTable::empty();

            // Load the values into the symbol table
            let cells               = talk_context.cell_block_mut(cell_block);
            for (symbol, value) in symbols {
                let pos                     = symbol_table.define_symbol(symbol);
                cells[pos.cell as usize]    = value;
            }

            // Run the continuation with our new table
            // TODO: release the cell block when the continuation returns
            create_continuation(Arc::new(Mutex::new(symbol_table)), vec![cell_block])
        }));

        self.run(continuation)
    }
}

impl TalkReleasableOwner<TalkValue> for TalkOwnedByRuntime {
    fn release_value(&self, value: TalkValue) {
        // Add to the list of released values, to be freed up later
        self.released_values.lock().unwrap().push(value);
    }
}

impl ArcWake for BackgroundFutureWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // Mark the future as awake and retrieve the waker
        let waker = {
            let mut background_tasks = arc_self.background_tasks.lock().unwrap();
            background_tasks.awake_futures.insert(arc_self.future_id);
            background_tasks.waker.take()
        };

        // Wake up the background task if needed
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

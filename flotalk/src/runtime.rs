use super::context::*;
use super::continuation::*;
use super::error::*;
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
use futures::task::{Poll, Context};

use flo_stream::*;

use std::sync::*;

// TODO: write and upgrade to a 'fair' mutex that processing wakeups in the order that they happen

///
/// A `TalkRuntime` is used to run continuations inside a `TalkContext` (it wraps a TalkContext,
/// and schedules continuations on them)
///
pub struct TalkRuntime {
    pub (crate) context: Arc<lock::Mutex<TalkContext>>
}

impl TalkRuntime {
    ///
    /// Creates a runtime for a context
    ///
    pub fn with_context(context: TalkContext) -> TalkRuntime {
        TalkRuntime {
            context: Arc::new(lock::Mutex::new(context))
        }
    }

    ///
    /// Returns an empty runtime
    ///
    pub fn empty() -> TalkRuntime {
        Self::with_context(TalkContext::empty())
    }

    ///
    /// Sends a message to a value using this runtime
    ///
    pub fn send_message<'a>(&'a self, value: &'a TalkValue, message: TalkMessage) -> impl 'a + Send + Future<Output=TalkValue> {
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
            self.run(TalkContinuation::Soon(Box::new(move |talk_context| {
                value.remove_reference(talk_context);
                TalkValue::Nil.into()
            }))).await;
        }
    }

    ///
    /// Runs a continuation with a 'later' part
    ///
    fn run_continuation_later<'a>(&self, later: Box<dyn 'a + Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkValue>>) -> impl 'a + Send + Future<Output=TalkValue> {
        // If the runtime is dropped while the future is running, it will be aborted (if it ever wakes up again)
        let talk_context        = Arc::downgrade(&self.context);
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

                Poll::Ready(TalkValue::Error(TalkError::RuntimeDropped))
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
    /// an error. If any of the messages generate an error, the rest of the stream will be discarded.
    ///
    pub fn stream_to<'a, TStream>(&'a self, create_receiver: impl Into<TalkContinuation<'a>>, stream: TStream) -> impl 'a + Send + Future<Output=Result<(), TalkError>> 
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
            if let TalkValue::Error(error) = &target {
                let error = error.clone();
                target.release_in_context(&*self.context.lock().await);
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
                if let TalkValue::Error(error) = &result {
                    let error = error.clone();

                    result.release_in_context(&*self.context.lock().await);
                    target.release_in_context(&*self.context.lock().await);

                    return Err(error);
                }

                // Release the resulting value
                result.release_in_context(&*self.context.lock().await);
            }

            // Stream was consumed
            target.release_in_context(&*self.context.lock().await);

            Ok(())
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
    pub fn stream_from<'a, TStreamItem>(&'a self, receive_target: impl Into<TalkContinuation<'a>>) -> impl 'a + Send + Stream<Item=Result<TStreamItem, TalkError>> + TryStream<Ok=TStreamItem, Error=TalkError>
    where
        TStreamItem: 'static + Send + TalkMessageType,
    {
        use futures::future::{Either};
        static VALUE_COLON_MSG: Lazy<TalkSymbol>  = Lazy::new(|| "value:".into());

        let context         = Arc::clone(&self.context);
        let receive_target  = receive_target.into();

        generator_stream(move |yield_value| {
            async move {
                // Create the sender object and the receiver stream
                let (sender, receiver)  = {
                    let mut context         = context.lock().await;
                    let (sender, receiver)  = create_talk_sender::<TStreamItem>(&mut *context);
                    (sender.leak(), receiver)
                };

                // Evaluate the value that we'll send the sender object to
                let receive_target = self.run(receive_target).await;
                if let TalkValue::Error(err) = &receive_target {
                    // Stop early if the target is an error
                    sender.release_in_context(&*context.lock().await);
                    yield_value(Err(err.clone())).await;
                    return;
                }

                // Start sending the 'value:' message (this runs in parallel with our relay code)
                let send_message = receive_target.send_message_in_context(TalkMessage::with_arguments(vec![(*VALUE_COLON_MSG, sender)]), &*context.lock().await);
                let send_message = self.run(send_message);

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
                        if let TalkValue::Error(err) = &send_message_result {
                            // Abort early and report the error
                            // yield_value(Err(err.clone())); -- TODO
                            send_message_result.release_in_context(&*context.lock().await);
                            return;
                        } else {
                            // Otherwise, release the result and wait for the relay to finish
                            send_message_result.release_in_context(&*context.lock().await);
                            relay_message.await;
                        }
                    }

                    Either::Right((_, send_message)) => {
                        // The relay finished but the send_message call is still going. Close the stream once the call finishes
                        let send_message_result = send_message.await;

                        if let TalkValue::Error(err) = &send_message_result {
                            // In spite of the stream being finished at this point, send_message errored anyway, so we'll report that
                            // yield_value(Err(err.clone())); -- TODO
                            send_message_result.release_in_context(&*context.lock().await);
                            return;
                        }
                    }
                }
            }
        })
    }

    ///
    /// Runs a continuation or a script using this runtime
    ///
    pub fn run<'a>(&self, continuation: impl Into<TalkContinuation<'a>>) -> impl 'a + Send + Future<Output=TalkValue> {
        let continuation = continuation.into();

        enum NowLater<T> {
            Now(TalkValue),
            Later(T),
        }

        let now_later = match continuation {
            TalkContinuation::Ready(value)  => NowLater::Now(value),
            TalkContinuation::Later(later)  => NowLater::Later(self.run_continuation_later(later)),

            TalkContinuation::Soon(soon)    => {
                let mut continuation = Some(TalkContinuation::Soon(soon));

                NowLater::Later(self.run_continuation_later(Box::new(move |talk_context, future_context| {
                    loop {
                        match continuation.take() {
                            None                                        => { return Poll::Ready(TalkValue::Nil); }
                            Some(TalkContinuation::Ready(val))          => { return Poll::Ready(val); }
                            Some(TalkContinuation::Later(mut later_fn)) => {
                                let result      = later_fn(talk_context, future_context);
                                continuation    = Some(TalkContinuation::Later(later_fn));

                                return result;
                            }

                            Some(TalkContinuation::Soon(soon_fn)) => {
                                continuation = Some(soon_fn(talk_context));
                            }
                        }
                    }
                })))
            },
        };

        async move {
            match now_later {
                NowLater::Now(value)    => value,
                NowLater::Later(later)  => later.await,
            }
        }
    }

    ///
    /// Generates a symbol table, then runs a continuation with it
    ///
    pub fn run_with_symbols<'a>(&'a self, create_symbol_table: impl 'a + Send + FnOnce(&mut TalkContext) -> Vec<(TalkSymbol, TalkValue)>, create_continuation: impl 'a + Send + FnOnce(Arc<Mutex<TalkSymbolTable>>, Vec<TalkCellBlock>) -> TalkContinuation<'static>) -> impl 'a + Send + Future<Output=TalkValue> {
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

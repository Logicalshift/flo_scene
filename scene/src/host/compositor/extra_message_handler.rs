use crate::compositor::either_message::*;
use crate::host::input_stream::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::subprogram_id::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{Waker};

use std::sync::*;

pub trait WithExtraMessageHandler<TMessage, TExtraMessage, TOldFuture> {
    ///
    /// Turns a subprogram function into one that can handle an extra message type
    ///
    fn with_extra_message_handler<TFuture>(self, extra_message: impl 'static + Send + FnOnce(InputStream<TExtraMessage>, SceneContext, MessageForwarder<TMessage>) -> TFuture) -> impl 'static + FnOnce(InputStream<EitherMessage<TMessage, TExtraMessage>>, SceneContext) -> BoxFuture<'static, ()>
    where
        TFuture: 'static + Send + Future<Output=()>;
}

///
/// Allows forwarding messages from an extra message program to the original program
///
pub struct MessageForwarder<TMessage> {
    program_id: SubProgramId,
    core:       Arc<Mutex<InputStreamCore<TMessage>>>,
}

impl<TMessage> Clone for MessageForwarder<TMessage> {
    #[inline]
    fn clone(&self) -> Self {
        Self { 
            program_id: self.program_id.clone(), 
            core:       self.core.clone(),
        }
    }
}

///
/// Creates a future that wakes up the specified waker every time the input core becomes idle
///
fn with_wake_when_idle<'a, TMessage>(future: impl 'a + Send + Future<Output=()>, input_core: Arc<Mutex<InputStreamCore<TMessage>>>, when_idle: Arc<Mutex<Option<Waker>>>) -> impl 'a + Send + Future<Output=()> 
where 
    TMessage: SceneMessage,
{
    let mut future = Box::pin(future);

    future::poll_fn(move |context| {
        // Poll the future
        let result = future.poll_unpin(context);

        // Check if the stream is idle
        let waker = {
            let input_core = input_core.lock().unwrap();

            if input_core.is_idle() {
                when_idle.lock().unwrap().take()                
            } else {
                None
            }
        };

        // If it is idle and there's a waker, then awaken the waker
        if let Some(waker) = waker {
            waker.wake();
        }

        result
    })
}

///
/// Creates a future that waits for the specified input core to become idle (signalled via the 'when idle' waker)
///
fn on_idle<TMessage>(input_core: Arc<Mutex<InputStreamCore<TMessage>>>, when_idle: Arc<Mutex<Option<Waker>>>) -> impl 'static + Send + Future<Output=()> 
where 
    TMessage: SceneMessage,
{
    future::poll_fn(move |context| {
        use futures::task::{Poll};

        let input_core = input_core.lock().unwrap();
        if input_core.is_idle() {
            // Finish when the core is idle
            Poll::Ready(())
        } else {
            // Wake up when the core becomes idle
            (*when_idle.lock().unwrap()) = Some(context.waker().clone());

            // Sleep until that happens
            Poll::Pending
        }
    })
}

impl<TFn, TMessage, TExtraMessage, TOldFuture> WithExtraMessageHandler<TMessage, TExtraMessage, TOldFuture> for TFn
where
    TFn:            'static + Send + FnOnce(InputStream<TMessage>, SceneContext) -> TOldFuture,
    TMessage:       SceneMessage,
    TExtraMessage:  SceneMessage,
    TOldFuture:     Send + Future<Output=()>,
{
    fn with_extra_message_handler<TFuture>(self, extra_message: impl 'static + Send + FnOnce(InputStream<TExtraMessage>, SceneContext, MessageForwarder<TMessage>) -> TFuture) -> impl 'static + FnOnce(InputStream<EitherMessage<TMessage, TExtraMessage>>, SceneContext) -> BoxFuture<'static, ()>
    where
        TFuture: 'static + Send + Future<Output=()>,
    {
        |input, context| async move {
            let Some(program_id)    = context.current_program_id() else { return; };
            let Some(scene_core)    = context.scene_core().upgrade() else { return; };

            // Create left and right input streams
            // Both have 0 slots (buffering only happens in the outer core)
            // TODO: these streams won't mark the scene as 'not idle' while they have queued messages
            let left        = InputStream::<TMessage>::new(program_id.clone(), &scene_core, 0);
            let right       = InputStream::<TExtraMessage>::new(program_id.clone(), &scene_core, 0);
            let left_core   = left.core();
            let right_core  = right.core();

            // Create the futures for the two sides
            let left_forwarder = MessageForwarder {
                program_id: program_id.clone(),
                core:       left_core.clone(),
            };
            let right_forwarder = MessageForwarder {
                program_id: program_id.clone(),
                core:       right_core.clone(),
            };

            let left    = (self)(left, context.clone());
            let right   = (extra_message)(right, context.clone(), left_forwarder.clone());

            // Change the futures into ones that reawaken the message consumer when they're idle
            let wake_when_idle  = Arc::new(Mutex::new(None));
            let left            = with_wake_when_idle(left, left_core.clone(), wake_when_idle.clone());
            let right           = with_wake_when_idle(right, right_core.clone(), wake_when_idle.clone());

            // Listen to all the futures, forwarding each message as it's received to the appropriate inbox for the two streams
            let main_core_1 = input.core();
            let main_core_2 = input.core();
            future::join3(
                async move {
                    // Wait for the left future
                    left.await;

                    // When the left future finishes, stop the main input (which will cause the whole program to stop eventually)
                    main_core_1.lock().unwrap().close();
                },
                async move {
                    // Wait for the right future
                    right.await;

                    // Stop the whole program when the future stops
                    main_core_2.lock().unwrap().close();
                },
                async move {
                    let mut input = input.messages_with_sources();

                    // Forward messages to either of the two futures
                    while let Some((source, msg)) = input.next().await {
                        // After forwarding a message, we wait for the relevant input stream to become idle again before sending the next message (this stops us from processing two messages
                        // in parallel, and also keeps the overall subprogram in a 'busy' state)
                        match msg {
                            EitherMessage::Left(msg)    => { left_forwarder.forward_with_sender(msg, source).await; on_idle(left_core.clone(), wake_when_idle.clone()).await; },
                            EitherMessage::Right(msg)   => { right_forwarder.forward_with_sender(msg, source).await; on_idle(right_core.clone(), wake_when_idle.clone()).await; },
                        }
                    }

                    left_core.lock().unwrap().close();
                    right_core.lock().unwrap().close();
                }
            ).await;
        }.boxed()
    }
}

impl<TMessage> MessageForwarder<TMessage>
where
    TMessage: SceneMessage,
{
    ///
    /// Sends a message to the input queue for the other kind of message
    ///
    pub async fn forward<'a>(&'a self, message: TMessage) {
        self.forward_with_sender(message, self.program_id.clone()).await;
    }

    ///
    /// Sends a message to the input queue for the other kind of message
    ///
    pub (crate) fn forward_with_sender<'a>(&'a self, message: TMessage, sender: SubProgramId) -> impl 'a + Send + Future<Output=()> {
        let mut message = Some(message);

        future::poll_fn(move |ctxt| {
            use futures::task::{Poll};

            // Borrow the message we're trying to send
            let Some(sending_message) = message.take() else { return Poll::Ready(()); };

            // Try to send to the core
            let mut core = self.core.lock().unwrap();
            
            match core.send(sender, sending_message) {
                Ok(waker) => {
                    // Message was queued
                    drop(core);

                    if let Some(waker) = waker { waker.wake(); }

                    Poll::Ready(())
                }

                Err(unsent_message) => {
                    // Stream is not ready to receive this message yet
                    if core.is_closed() {
                        // Just drop the message if the core is closed
                        Poll::Ready(())
                    } else if core.is_waiting_for_idle() {
                        // Should not happen: context.wait_for_idle does not block this input stream
                        panic!("Core waiting for idle")
                    } else {
                        // Requeue the message, wake when the core is ready to receive
                        message = Some(unsent_message);
                        core.wake_when_slots_available(ctxt);
                        Poll::Pending
                    }
                }
            }
        })
    }
}

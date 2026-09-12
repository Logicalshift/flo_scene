use crate::compositor::either_message::*;
use crate::host::input_stream::*;
use crate::host::scene_context::*;
use crate::host::scene_message::*;
use crate::host::subprogram_id::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

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

            // Listen to all the futures, forwarding each message as it's received to the appropriate inbox for the two streams
            let main_core_1 = input.core();
            let main_core_2 = input.core();
            future::join3(
                async move {
                    left.await;
                    main_core_1.lock().unwrap().close();
                },
                async move {
                    right.await;
                    main_core_2.lock().unwrap().close();
                },
                async move {
                    let mut input = input.messages_with_sources();

                    while let Some((source, msg)) = input.next().await {
                        match msg {
                            EitherMessage::Left(msg)    => left_forwarder.forward_with_sender(msg, source).await,
                            EitherMessage::Right(msg)   => right_forwarder.forward_with_sender(msg, source).await,
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

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
            let left        = InputStream::<TMessage>::new(program_id.clone(), &scene_core, 0);
            let right       = InputStream::<TExtraMessage>::new(program_id.clone(), &scene_core, 0);
            let left_core   = left.core();
            let right_core  = right.core();

            // Create the futures for the two sides
            let left_forwarder = MessageForwarder {
                program_id: program_id.clone(),
                core:       left_core,
            };
            let right_forwarder = MessageForwarder {
                program_id: program_id.clone(),
                core:       right_core,
            };

            let left    = (self)(left, context.clone());
            let right   = (extra_message)(right, context.clone(), left_forwarder.clone());

            // Listen to all the futures, forwarding each message as it's received to the appropriate inbox for the two streams
            future::select_all([
                left.boxed(),
                right.boxed(),
                async move {
                    let mut input = input;

                    while let Some(msg) = input.next().await {
                        match msg {
                            EitherMessage::Left(msg)    => left_forwarder.forward(msg).await,
                            EitherMessage::Right(msg)   => right_forwarder.forward(msg).await,
                        }
                    }
                }.boxed()
            ]).await;
        }.boxed()
    }
}

impl<TMessage> MessageForwarder<TMessage>
where
    TMessage: SceneMessage,
{
    pub fn forward<'a>(&'a self, message: TMessage) -> impl 'a + Send + Future<Output=()> {
        let mut message = Some(message);

        future::poll_fn(move |ctxt| {
            use futures::task::{Poll};

            // Borrow the message we're trying to send
            let Some(sending_message) = message.take() else { return Poll::Ready(()); };

            // Try to send to the core
            let program_id  = self.program_id.clone();
            let mut core    = self.core.lock().unwrap();
            
            match core.send(program_id, sending_message) {
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

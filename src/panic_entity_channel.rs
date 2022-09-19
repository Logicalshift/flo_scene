use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;

use futures::prelude::*;
use futures::stream;
use futures::future::{BoxFuture};
use futures::channel::oneshot;

use std::thread;

///
/// Entity channel that sends a message on a stream if it is dropped by a panicking thread
///
pub struct PanicEntityChannel<TChannel>
where
    TChannel: EntityChannel,
{
    /// The entity channel that this will send messages to
    channel: TChannel,

    /// The sender for the panic message
    send_panic: Option<oneshot::Sender<TChannel::Message>>,

    /// The message to send when the channel panics (if it has not been sent yet)
    panic_message: Option<TChannel::Message>,
}

impl<TChannel> PanicEntityChannel<TChannel> 
where
    TChannel:           EntityChannel,
    TChannel::Message:  'static,
{
    ///
    /// Creates a new panic entity channel. The supplied stream is modified to receive the panic message, should it occur
    ///
    pub fn new(source_channel: TChannel, stream: impl 'static + Send + Stream<Item=TChannel::Message>, panic_message: TChannel::Message) -> (PanicEntityChannel<TChannel>, impl 'static + Send + Stream<Item=TChannel::Message>) {
        // Create a oneshot receiver for the panic message, and 
        let (sender, receiver)  = oneshot::channel();
        let receiver            = receiver.map(|maybe_result| {
            match maybe_result {
                Ok(msg) => stream::iter(vec![msg]),
                Err(_)  => stream::iter(vec![]),
            }
        }).flatten_stream();

        // Amend the existing stream
        let stream = stream::select(stream, receiver);

        // Create the resulting channel
        let entity_channel = PanicEntityChannel {
            channel:        source_channel,
            send_panic:     Some(sender),
            panic_message:  Some(panic_message),
        };

        (entity_channel, stream)
    }
}

impl<TChannel> EntityChannel for PanicEntityChannel<TChannel>
where
    TChannel: EntityChannel,
{
    type Message = TChannel::Message;

    #[inline]
    fn entity_id(&self) -> EntityId { 
        self.channel.entity_id()
    }

    #[inline]
    fn is_closed(&self) -> bool {
        self.channel.is_closed()
    }

    #[inline]
    fn send(&mut self, message: Self::Message) -> BoxFuture<'static, Result<(), EntityChannelError>> {
        self.channel.send(message)
    }
}

impl<TChannel> Drop for PanicEntityChannel<TChannel> 
where
    TChannel: EntityChannel,
{
    fn drop(&mut self) {
        if thread::panicking() {
            if let (Some(send_panic), Some(panic_message)) = (self.send_panic.take(), self.panic_message.take()) {
                send_panic.send(panic_message).ok();
            }
        }
    }
}

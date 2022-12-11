use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;

use futures::prelude::*;
use futures::stream;
use futures::future::{BoxFuture};
use futures::channel::oneshot;

use std::thread;
use std::cell::{RefCell};
use std::sync::atomic::{AtomicBool, Ordering};
use std::panic;

thread_local! {
    ///
    /// The last panic message captured by the hook
    ///
    static LAST_PANIC_MESSAGE: RefCell<Option<String>> = RefCell::new(None);

    ///
    /// The last panic location captured by the hook
    ///
    static LAST_PANIC_LOCATION: RefCell<Option<String>> = RefCell::new(None);
}

///
/// Set to true once we've installed a panic hook
///
static PANIC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

///
/// Installs the panic hook if it's not already installed (to retain a copy of the last panic message to return to callers)
///
fn install_panic_hook() {
    if !PANIC_HOOK_INSTALLED.swap(true, Ordering::Acquire) {
        // Run the old behaviour after the new behaviour
        let old_hook = panic::take_hook();

        // Set a new panic hook to set the LAST_PANIC_MESSAGE
        panic::set_hook(Box::new(move |info| {
            LAST_PANIC_MESSAGE.try_with(|last_panic_message| {
                let last_panic_message = last_panic_message.try_borrow_mut();

                if let Ok(mut last_panic_message) = last_panic_message {
                    // Retrieve the payload as a string, if possible, and store in the last panic message
                    let payload = info.payload();

                    if let Some(str_value) = payload.downcast_ref::<&str>() {
                        *last_panic_message = Some(str_value.to_string());
                    } else if let Some(string_value) = payload.downcast_ref::<String>() {
                        *last_panic_message = Some(string_value.clone());
                    } else {
                        // There is a message but we don't know how to convert it to string
                        *last_panic_message = None;
                    }
                }
            }).ok();

            LAST_PANIC_LOCATION.try_with(|last_panic_location| {
                if let Ok(mut last_panic_location) = last_panic_location.try_borrow_mut() {
                    if let Some(location) = info.location() {
                        *last_panic_location = Some(format!("{}: {}:{}", location.file(), location.line(), location.column()));
                    } else {
                        *last_panic_location = None;
                    }
                }
            }).ok();

            old_hook(info);
        }));
    }
}

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
    panic_message: Option<Box<dyn Send + FnOnce(String, Option<String>) -> TChannel::Message>>,
}

impl<TChannel> PanicEntityChannel<TChannel> 
where
    TChannel:           EntityChannel,
    TChannel::Message:  'static,
{
    ///
    /// Creates a new panic entity channel. The supplied stream is modified to receive the panic message, should it occur
    ///
    pub fn new(source_channel: TChannel, stream: impl 'static + Send + Stream<Item=TChannel::Message>, panic_message: impl 'static + Send + FnOnce(String, Option<String>) -> TChannel::Message) -> (PanicEntityChannel<TChannel>, impl 'static + Send + Stream<Item=TChannel::Message>) {
        // Ensure that this panic hook is installed (so that LAST_PANIC_MESSAGE is updated)
        install_panic_hook();

        // Create a oneshot receiver for the panic message
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
            panic_message:  Some(Box::new(panic_message)),
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
                let last_panic_location = LAST_PANIC_LOCATION.try_with(|loc| loc.borrow_mut().take()).unwrap_or(None);
                let last_panic_string   = LAST_PANIC_MESSAGE.try_with(|msg| msg.borrow_mut().take()).unwrap_or(None);
                let last_panic_string   = last_panic_string.unwrap_or_else(|| "<NO PANIC MESSAGE AVAILABLE>".to_string());

                send_panic.send(panic_message(last_panic_string, last_panic_location)).ok();
            }
        }
    }
}

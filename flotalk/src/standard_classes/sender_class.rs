use crate::allocator::*;
use crate::class::*;
use crate::context::*;
use crate::continuation::*;
use crate::error::*;
use crate::message::*;
use crate::reference::*;
use crate::releasable::*;
use crate::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use futures::prelude::*;
use futures::channel::mpsc;
use futures::lock;

use std::any::{TypeId};
use std::marker::{PhantomData};
use std::collections::{HashMap};
use std::sync::*;

static SENDER_CLASS: Lazy<Mutex<HashMap<TypeId, TalkClass>>> = Lazy::new(|| Mutex::new(HashMap::new()));

///
/// The sender class is a class that sends all of its messages to a stream
///
pub struct TalkSenderClass<TItem>
where
    TItem: 'static + Send + TalkMessageType,
{
    sender: PhantomData<Arc<lock::Mutex<mpsc::Sender<TItem>>>>,
}

///
/// An instance of the TalkSenderClass
///
pub struct TalkSender<TItem>
where
    TItem: Send + TalkMessageType,
{
    sender: Arc<lock::Mutex<mpsc::Sender<TItem>>>,
}

impl<TItem> TalkReleasable for TalkSender<TItem>
where
    TItem: Send + TalkMessageType,
{
    #[inline]
    fn release_in_context(self, _context: &TalkContext) { }
}

impl<TItem> TalkClassDefinition for TalkSenderClass<TItem>
where
    TItem: 'static + Send + TalkMessageType,
{
    type Data       = TalkSender<TItem>;
    type Allocator  = TalkStandardAllocator<Self::Data>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        TalkStandardAllocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _arguments: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message_id)))
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, arguments: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        // Turn the arguments back into a message
        let message = if arguments.len() == 0 {
            TalkMessage::Unary(message_id)
        } else {
            TalkMessage::WithArguments(message_id, arguments.leak())
        };

        // Take a copy of the sender for the continuation
        let mut allocator   = allocator.lock().unwrap();
        let target          = allocator.retrieve(reference.1);
        let sender          = target.sender.clone();

        // Create a continuation that sends the message
        TalkContinuation::soon(move |context| {
            // Convert the message to the stream
            let message = TalkOwned::new(message, &*context);
            let item    = TItem::from_message(message, context);

            // Result is an error if we can't convert the message, or a continuation that sends to the sender
            match item {
                Err(err)    => err.into(),
                Ok(item)    => TalkContinuation::future_value(async move { sender.lock().await.send(item).await.ok(); TalkValue::Nil })
            }
        })
    }
}

///
/// Retrieves (or creates) the TalkClass corresponding to a `TalkSenderClass<TItem>` (ie, a class that writes messages of that
/// type to a stream)
///
pub (crate) fn talk_sender_class<TItem>() -> TalkClass 
where
    TItem: 'static + Send + TalkMessageType,
{
    let mut sender_classes = SENDER_CLASS.lock().unwrap();

    if let Some(class) = sender_classes.get(&TypeId::of::<TItem>()) {
        // This class was already created/registered
        *class
    } else {
        // This item type hasn't been seen before: register a new class and return it
        let class = TalkClass::create(TalkSenderClass::<TItem> { sender: PhantomData });
        sender_classes.insert(TypeId::of::<TItem>(), class);

        class
    }
}

///
/// Creates a sender object and a receiver stream for a specific item type
///
pub fn create_talk_sender_in_context<'a, TItem>(talk_context: &'a mut TalkContext) -> (TalkOwned<TalkReference, &'a TalkContext>, impl Send + Stream<Item=TItem>)
where
    TItem: 'static + Send + TalkMessageType,
{
    // Get the class to create and the allocator
    let sender_class    = talk_sender_class::<TItem>();
    let allocator       = talk_context.get_callbacks_mut(sender_class).allocator::<TalkStandardAllocator<TalkSender<TItem>>>().unwrap();

    // Create a sender and a receiver
    let (send, receive) = mpsc::channel(1);

    // Create the sender and store it using the allocator
    let sender              = TalkSender { sender: Arc::new(lock::Mutex::new(send)) };
    let sender_data_handle  = allocator.lock().unwrap().store(sender);

    // Generate the result
    let sender              = TalkReference(sender_class, sender_data_handle);
    let sender              = TalkOwned::new(sender, &*talk_context);

    (sender, receive)
}

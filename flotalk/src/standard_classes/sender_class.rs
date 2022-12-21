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
    fn release_in_context(self, context: &TalkContext) { }
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
    fn create_allocator(&self) -> Self::Allocator {
        TalkStandardAllocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _arguments: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message_id)))
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, arguments: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, _reference: TalkReference, target: &mut Self::Data) -> TalkContinuation<'static> {
        // Turn the arguments back into a message
        let message = if arguments.len() == 0 {
            TalkMessage::Unary(message_id)
        } else {
            TalkMessage::WithArguments(message_id, arguments.leak())
        };

        // Take a copy of the sender for the continuation
        let sender = target.sender.clone();

        // Create a continuation that sends the message
        TalkContinuation::soon(move |context| {
            // Convert the message to the stream
            let message = TalkOwned::new(message, context);
            let item    = TItem::from_message(message, context);

            // Result is an error if we can't convert the message, or a continuation that sends to the sender
            match item {
                Err(err)    => err.into(),
                Ok(item)    => TalkContinuation::future(async move { sender.lock().await.send(item).await; TalkValue::Nil })
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

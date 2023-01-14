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
use futures::lock;

use std::any::{TypeId};
use std::marker::{PhantomData};
use std::collections::{HashMap};
use std::sync::*;

/// The 'next' message is sent to a receiver to request the next message, or nil if there is no following message
static TALK_MSG_NEXT: Lazy<TalkMessageSignatureId> = Lazy::new(|| "next".into());

/// Maps a receiver class for a particular stream type
static RECEIVER_CLASS: Lazy<Mutex<HashMap<TypeId, TalkClass>>> = Lazy::new(|| Mutex::new(HashMap::new()));

///
/// The sender class is a class that receives all its items from a stream
///
pub struct TalkReceiverClass<TStream>
where
    TStream: 'static + Send + Unpin + Stream<Item=TalkMessage>,
{
    stream: PhantomData<Arc<lock::Mutex<TStream>>>,
}

pub struct TalkReceiver<TStream>
where
    TStream: 'static + Send + Unpin + Stream<Item=TalkMessage>,
{
    stream: Arc<Mutex<Option<TStream>>>,
}


impl<TStream> TalkReleasable for TalkReceiver<TStream>
where
    TStream: 'static + Send + Unpin + Stream<Item=TalkMessage>,
{
    #[inline]
    fn release_in_context(self, _context: &TalkContext) { }
}

impl<TStream> TalkClassDefinition for TalkReceiverClass<TStream>
where
    TStream: 'static + Send + Unpin + Stream<Item=TalkMessage>,
{
    type Data       = TalkReceiver<TStream>;
    type Allocator  = TalkStandardAllocator<Self::Data>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Self::Allocator {
        TalkStandardAllocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _arguments: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _arguments: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        if message_id == *TALK_MSG_NEXT {
            // Create a continuation to read the next value from the receiver: this borrows the receiver while it's running so 
            let mut allocator       = allocator.lock().unwrap();
            let receiver            = allocator.retrieve(reference.data_handle());

            // Take the stream out of the container
            let stream_container    = Arc::clone(&receiver.stream);
            let stream              = stream_container.lock().unwrap().take();

            if let Some(stream) = stream {
                // Create a future to read from the stream, and put the value back into the container
                TalkContinuation::future_value(async move {
                    // Read the next value from the stream
                    let mut stream  = stream;
                    let next_value  = stream.next().await;

                    // Replace the stream
                    *stream_container.lock().unwrap() = Some(stream);

                    // Return the value
                    if let Some(message) = next_value {
                        TalkValue::Message(Box::new(message))
                    } else {
                        TalkValue::Nil
                    }
                })
            } else {
                // Stream is busy if it's not in the container at this point (we only allow one task to read from it at once)
                TalkError::Busy.into()
            }
        } else {
            // No other messages are supported
            TalkError::MessageNotSupported(message_id).into()
        }
    }
}

///
/// Retrieves (or creates) the TalkClass corresponding to a `TalkReceiverClass<TItem>` (a class that can be used to read asynchronously from a stream)
///
pub (crate) fn talk_receiver_class<TStream>() -> TalkClass 
where
    TStream: 'static + Send + Unpin + Stream<Item=TalkMessage>,
{
    let mut receiver_classes = RECEIVER_CLASS.lock().unwrap();

    if let Some(class) = receiver_classes.get(&TypeId::of::<TStream>()) {
        // This class was already created/registered
        *class
    } else {
        // This item type hasn't been seen before: register a new class and return it
        let class = TalkClass::create(TalkReceiverClass::<TStream> { stream: PhantomData });
        receiver_classes.insert(TypeId::of::<TStream>(), class);

        class
    }
}

///
/// Creates a receiver object for a message stream
///
pub fn create_talk_receiver<'a, TStream>(message_stream: TStream, talk_context: &'a mut TalkContext) -> TalkOwned<TalkReference, &'a TalkContext> 
where
    TStream: 'static + Send + Unpin + Stream<Item=TalkMessage>,
{
    // Get the class to create and the allocator
    let receiver_class  = talk_receiver_class::<TStream>();
    let allocator       = talk_context.get_callbacks_mut(receiver_class).allocator::<TalkStandardAllocator<_>>().unwrap();

    // Create the receiver and store it using the allocator
    let receiver                = TalkReceiver { stream: Arc::new(Mutex::new(Some(message_stream))) };
    let receiver_data_handle    = allocator.lock().unwrap().store(receiver);

    // Generate the result
    let receiver = TalkReference(receiver_class, receiver_data_handle);
    let receiver = TalkOwned::new(receiver, &*talk_context);

    receiver
}

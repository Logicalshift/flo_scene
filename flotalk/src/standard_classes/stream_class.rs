use super::sender_class::*;
use super::receiver_class::*;

use crate::allocator::*;
use crate::context::*;
use crate::class::*;
use crate::continuation::*;
use crate::dispatch_table::*;
use crate::error::*;
use crate::message::*;
use crate::reference::*;
use crate::releasable::*;
use crate::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::sync::*;

/// The 'stream' class, creates asynchronous generator style sender/receiver streams
pub static STREAM_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkStreamClass { }));

///
/// The `Stream` class, which can be used to receive values from an asynchronous stream
///
/// A stream can be created like this: `someReceiver := Stream withSender: [ :messageSender | messageSender hello: foo ].`
/// Here, `someReceiver` will be a receiver object and `messageSender` 
///
pub struct TalkStreamClass {

}

impl TalkClassDefinition for TalkStreamClass {
    /// The type of the data stored by an object of this class (this particular class is never instantiated)
    type Data = ();

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkStandardAllocator<()>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self) -> Self::Allocator {
        Self::Allocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, class_id: TalkClass, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        static TALK_MSG_WITHSENDER: Lazy<TalkMessageSignatureId>    = Lazy::new(|| ("withSender:").into());
        static TALK_MSG_WITHRECEIVER: Lazy<TalkMessageSignatureId>  = Lazy::new(|| ("withReceiver:").into());
        static TALK_MSG_VALUE: Lazy<TalkMessageSignatureId>         = Lazy::new(|| ("value:").into());

        if message_id == *TALK_MSG_WITHSENDER {
            // The first argument is the sender block
            let mut args        = args;
            let sender_block    = args[0].take();

            TalkContinuation::soon(move |context| {
                // Create a sender and a receiver
                let (sender_value, receiver_stream) = create_talk_sender::<TalkMessage>(context);
                let sender_value                    = sender_value.leak();
                let receiver                        = create_talk_receiver(receiver_stream, context);
                let receiver                        = receiver.leak();

                // Run the sender
                let run_sender = sender_block.send_message_in_context(TalkMessage::WithArguments(*TALK_MSG_VALUE, smallvec![sender_value.into()]), context);
                context.run_in_background(run_sender);

                // Result is the receiver
                receiver.into()
            })
        } else if message_id == *TALK_MSG_WITHRECEIVER {
            let mut args        = args;
            let receiver_block  = args[0].take();

            TalkContinuation::soon(move |context| {
                // Create a sender and a receiver
                let (sender_value, receiver_stream) = create_talk_sender::<TalkMessage>(context);
                let sender_value                    = sender_value.leak();
                let receiver                        = create_talk_receiver(receiver_stream, context);
                let receiver                        = receiver.leak();

                // Run the receiver
                let run_receiver = receiver_block.send_message_in_context(TalkMessage::WithArguments(*TALK_MSG_VALUE, smallvec![receiver.into()]), context);
                context.run_in_background(run_receiver);

                // Result is the sender
                sender_value.into()
            })
        } else {
            TalkError::MessageNotSupported(message_id).into()
        }
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}
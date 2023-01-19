use super::receiver_class::*;
use super::script_class::*;
use super::later_class::*;

use crate::allocator::*;
use crate::context::*;
use crate::class::*;
use crate::continuation::*;
use crate::error::*;
use crate::message::*;
use crate::reference::*;
use crate::releasable::*;
use crate::sparse_array::*;
use crate::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use futures::prelude::*;
use futures::channel::mpsc;

use std::sync::*;

/// The 'stream with reply' class, creates asynchronous generator style sender/receiver streams
pub (crate) static STREAM_WITH_REPLY_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkStreamWithReplyClass));

///
/// The `StreamWithReply` class, which is similar to the stream class, except every message that's sent is modified to have a return value
///
/// These streams can be created like this: 
/// ```SmallTalk
/// someSender := StreamWithReply withReceiver: [ :receiver | 
/// | nextMsg |
/// [
///     nextMsg ifMatches: #result:addOne: do: [ :result :val | result setValue: val + 1 ]
/// ] while: [
///     nextMsg := receiver next.
///     ^(nextMsg isNil) not
/// ].
/// ```
///
/// Sending `addOne:` to `someSender` will cause the stream to create a `Later` object and prepend it to the message, then waits for the
/// value to be populated. The message is changed from `#addOne:` to `#result:addOne:` by this process.
///
pub struct TalkStreamWithReplyClass;

///
/// The data storage structure for the TalkStreamWithReply class
///
pub struct TalkStreamWithReply {
    sender: mpsc::Sender<TalkMessage>,
}

impl TalkReleasable for TalkStreamWithReply {
    fn release_in_context(self, _context: &TalkContext) { }
}

///
/// Capitalizes the first letter of a string
///
fn capitalized(name: &str) -> String {
    let mut name_chrs = name.chars();

    if let Some(first_chr) = name_chrs.next() {
        first_chr.to_uppercase()
            .chain(name_chrs)
            .collect()
    } else {
        String::new()
    }
}

impl TalkClassDefinition for TalkStreamWithReplyClass {
    /// The type of the data stored by an object of this class (this particular class is never instantiated)
    type Data = TalkStreamWithReply;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkStandardAllocator<TalkStreamWithReply>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
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
            let mut args        = args;
            let sender_block    = args[0].take();

            // Create the stream object
            let (sender, receiver)  = mpsc::channel(1);
            let stream_object       = TalkStreamWithReply { sender };
            let stream_object       = allocator.lock().unwrap().store(stream_object);
            let stream_object       = TalkValue::Reference(TalkReference(class_id, stream_object));

            TalkContinuation::soon(move |context| {
                // Create the receiver object for this stream
                let receiver    = create_talk_receiver(receiver, context);
                let receiver    = receiver.leak();

                // Run the receiver
                let run_sender = sender_block.send_message_in_context(TalkMessage::WithArguments(*TALK_MSG_VALUE, smallvec![stream_object.into()]), context);
                context.run_in_background(run_sender);

                // Result is the receiver
                receiver.into()
            })

        } else if message_id == *TALK_MSG_WITHRECEIVER {
            let mut args        = args;
            let receiver_block  = args[0].take();

            let (sender, receiver)  = mpsc::channel(1);
            let stream_object       = TalkStreamWithReply { sender };
            let stream_object       = allocator.lock().unwrap().store(stream_object);
            let stream_object       = TalkValue::Reference(TalkReference(class_id, stream_object));

            TalkContinuation::soon(move |context| {
                // Create the receiver for this stream
                let receiver            = create_talk_receiver(receiver, context);
                let receiver            = receiver.leak();

                // Run the receiver
                let run_receiver = receiver_block.send_message_in_context(TalkMessage::WithArguments(*TALK_MSG_VALUE, smallvec![receiver.into()]), context);
                context.run_in_background(run_receiver);

                // Result is the stream
                stream_object.into()
            })

        } else if message_id == *TALK_MSG_SUBCLASS {

            TalkScriptClassClass::create_subclass(class_id, vec![*TALK_MSG_WITHSENDER, *TALK_MSG_WITHRECEIVER])

        } else {
            TalkError::MessageNotSupported(message_id).into()
        }
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        static TALK_MSG_NEW: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| ("new").into());
        static TALK_MSG_VALUE: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| ("value").into());
        static SIG_CACHE: Lazy<Mutex<TalkSparseArray<TalkMessageSignatureId>>>  = Lazy::new(|| Mutex::new(TalkSparseArray::empty()));
        static TALK_MSG_SENDER: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| ("sender").into());

        // Every message is sent to the sender with a new 'later' object attached
        let mut allocator   = allocator.lock().unwrap();
        let stream_object   = allocator.retrieve(reference.1);
        let mut sender      = stream_object.sender.clone();
        let args            = args.leak();

        // Create the 'later' object, adjust the message, send it to the sender and finally wait for the 'value' message to respond
        TalkContinuation::soon(move |talk_context| {
            // Create a new 'LATER' object
            LATER_CLASS.send_message_in_context(TalkMessage::Unary(*TALK_MSG_NEW), talk_context)
        }).and_then_soon(move |later_reference, talk_context| {
            // Fetch the signature with 'result' attached
            let mut sig_cache   = SIG_CACHE.lock().unwrap();
            let new_signature   = if let Some(new_sig) = sig_cache.get(message_id.into()) {
                *new_sig
            } else {
                let old_sig = message_id.to_signature();
                match old_sig {
                    TalkMessageSignature::Unary(symbol)   => {
                        // `#unary` -> `#resultForUnary`
                        let new_symbol_name = format!("resultFor{}:", capitalized(symbol.name()));

                        // Convert to a message sig ID
                        let new_sig = TalkMessageSignatureId::from(TalkMessageSignature::Arguments(smallvec![new_symbol_name.into()]));

                        // Store in the cache
                        sig_cache.insert(message_id.into(), new_sig);

                        new_sig
                    }

                    TalkMessageSignature::Arguments(mut args) => {
                        // `#withArgs:` -> `#result:withArgs:`
                        args.insert(0, "result:".into());

                        // Convert to a message sig ID
                        let new_sig = TalkMessageSignatureId::from(TalkMessageSignature::Arguments(args));

                        // Store in the cache
                        sig_cache.insert(message_id.into(), new_sig);

                        new_sig
                    }
                }
            };

            // Generate the new message using the signature
            let later_sender    = later_reference.clone_in_context(talk_context);
            let later_sender    = later_sender.send_message_in_context(TalkMessage::Unary(*TALK_MSG_SENDER), talk_context);

            // Start waiting for the result
            let wait_for_result = later_reference.send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), talk_context);

            // Send to the stream, then wait for the result
            later_sender.and_then_soon_if_ok(move |later_sender, _talk_context| {
                let mut args = args;
                args.insert(0, later_sender);

                let message = TalkMessage::WithArguments(new_signature, args);

                TalkContinuation::future_value(async move { sender.send(message).await.ok(); ().into() })
            }).and_then(move |_| wait_for_result)
        })
    }
}

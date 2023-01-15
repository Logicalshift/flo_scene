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

use std::sync::*;

///
/// The `Inverted` class provides a way to declare messages that are sent *from* an instance instead of *to* an instance.
///
/// Normally, the left-hand side of a 'message' expression such as `foo exampleMessage.` is the receiver of the message: the 
/// message is sent to the object `foo`. Messages that are declared on an `Inverted` class work the opposite way around, ie
/// for an inverted message, `foo` would be the sender and not the receiver. The receivers would be one or more instances of
/// a subclass of the `Inverted` class that implement the exampleMessage and which are configured to listen to the `foo`
/// object.
///
/// Another difference that 'inverted' messages have over the more traditional kind is that inverted messages know both the
/// sender (the object on the left-hand side of the expresssion), and the receiver (the `Inverted` instance that receives
/// the message). As there can be multiple receivers, inverted messages do not produce a return value.
///
/// This type is designed to support code structured using dependency inversion. For an example of what this means, consider
/// the problem of designing a logging framework. If messages are sent directly to a logging object, then every object that
/// needs to produce log messages has to have a reference to that object. This is the conventional dependency model: objects
/// need to 'log to' the logger object.
///
/// A logger implemented as a subclass of `Inverted` will instead use a 'log from' model for receiving its messages. Instead
/// of needing to tell every object about the logger, the logger is told about every object it should receive messages from.
/// This reverses the order of the dependencies, hence the name 'dependency inversion'. Typically this is used as a way for
/// a higher-level (more abstract) object to communicate with a lower level (more concrete) object or subsystem, where it
/// creates a more flexible and easier to understand dependency structure.
///
/// Here's how this works with the `Inverted` type. We can define our logger class like this:
///
/// ```SmallTalk
/// | Logger |
///
/// Logger := InvertedClass subclass.
///
/// Logger
///     addInstanceMessage: #logDebug: withAction: [ :msg :sender :self | "... write debug message ..." ];
///     addInstanceMessage: #logError: withAction: [ :msg :sender :self | "... write error message ..." ].
/// ```
///
/// So far this looks like a fairly normal class definition but these messages are not sent directly to an instance of the
/// `Logger` class but instead to any other object that wants to write out log messages:
///
/// ```SmallTalk
/// | object |
/// object := Object new.
/// object logDebug: 'Hello'.
/// ```
///
/// This will have no effect as no loggers are listening to the object we just declared, but also it won't produce a message
/// not supported error as the message is part of an `Inverted` class. We can create a logger to listen to this object, like
/// this:
///
/// ```SmallTalk
/// | someLogger |
/// someLogger := Logger new.
/// someLogger receiveFrom: object.
/// object logDebug: 'Hello'.
/// ```
///
/// This time, the logger receives the message sent by the object, as it has a dependency on it. Note that this is a weak
/// dependency; the object is not retained in memory by the logger.
///
/// This makes it a little easier to define custom loggers for types of object but a more realistic logger will probably
/// want to receive messages from all the objects, or perhaps all the objects that are stored in a particular context.
/// Conventially this can be acheived with a dependency injection framework, but FloTalk has explicit support for receiving
/// messages from groups of objects and blocks of code:
///
/// ```SmallTalk
/// someLogger receiveFrom: object.                             "Receive log messages from a specific object"
/// someLogger receiveFrom: all.                                "Receive log messages from everywhere"
/// someLogger with: [ "...code..." ].                          "Receive log messages from every object stored in a frame beneath a block"
/// someLogger withAsync: [ "...code..." ].                     "Receive log messages from every object stored in a frame beneath a block that is running asynchronously"
/// someLogger receiveFrom: all unreceived.                     "Receive log messages from every object whose message is not received by any other logger first"
/// someLogger receiveFrom: SomeClass.                          "Receive log messages the SomeClass class object itself"
/// someLogger receiveFrom: SomeClass instances.                "Receive log messages from every instance of SomeClass"
/// someLogger receiveFrom: SomeClass instances unreceived.     "Receive log messages from every instance of SomeClass if they are otherwise unreceived"
/// ```
///
/// The `Inverted` instance that received the most recent `receiveFrom:` or `with:` call will receive the message with the 
/// highest priority for the cases where that matters.
///
/// An inverted instance message can return the value `Inverted notHandled` if it wants to indicate that it doesn't want 
/// the message to be considered as 'received' for lower priority handlers.
///
pub struct TalkInvertedClass {

}

///
/// Data stored for an instance of the inverted class
///
pub struct TalkInverted {

}

///
/// When a particular inverted message should be sent to a type (if it has not been received by another processor or always)
///
#[derive(Copy, Clone)]
enum ProcessWhen {
    Always,
    Unreceived,    
}

///
/// The priority of an inverted receiver (higher priorities receive messages sooner)
///
#[derive(Copy, Clone)]
struct Priority(usize, ProcessWhen);

///
/// Allocator for instances of the inverted class
///
pub struct TalkInvertedClassAllocator {
    /// The priority of the next receiveFrom: implementation that's added
    next_priority: usize,

    /// The classes in the current context that can respond to each type of message ID (expectation is for there to be usually just one class per message)
    responder_classes: TalkSparseArray<SmallVec<[TalkClass; 2]>>,

    /// A Vec, indexed by responder class ID that contains the references of that class that want to respond to all
    /// Ie, for a given message to find out which 'all' responders are present, we look up the responder classes
    respond_to_all: Vec<Vec<(TalkReference, Priority)>>,

    /// A Vec, indexed by *source* class ID that contains a sparse array of source data handles and the `Inverted` references that respond to them
    respond_to_specific: Vec<TalkSparseArray<Vec<(TalkReference, Priority)>>>,

    /// The data store
    data: Vec<Option<TalkInverted>>,

    /// Reference counts for each allocated item in the data store (data is dropped when the count reaches 0)
    reference_counts: Vec<usize>,

    /// Items in the data array that have been freed and are available for reallocation
    free_slots: Vec<usize>,
}

impl TalkReleasable for TalkInverted {
    fn release_in_context(self, _context: &TalkContext) { }
}

impl TalkClassAllocator for TalkInvertedClassAllocator {
    /// The type of data stored for this class
    type Data = TalkInverted;

    ///
    /// Retrieves a reference to the data attached to a handle (panics if the handle has been released)
    ///
    #[inline]
    fn retrieve<'a>(&'a mut self, TalkDataHandle(pos): TalkDataHandle) -> &'a mut Self::Data {
        self.data[pos].as_mut().unwrap()
    }

    ///
    /// Adds to the reference count for a data handle
    ///
    #[inline]
    fn retain(allocator: &Arc<Mutex<Self>>, TalkDataHandle(pos): TalkDataHandle, _: &TalkContext) {
        let mut allocator = allocator.lock().unwrap();

        if allocator.reference_counts[pos] > 0 {
            allocator.reference_counts[pos] += 1;
        }
    }

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    #[inline]
    fn release(allocator: &Arc<Mutex<Self>>, TalkDataHandle(pos): TalkDataHandle, talk_context: &TalkContext) -> TalkReleaseAction {
        let freed_value = {
            let mut allocator = allocator.lock().unwrap();

            if allocator.reference_counts[pos] > 0 {
                allocator.reference_counts[pos] -= 1;

                if allocator.reference_counts[pos] == 0 {
                    allocator.free_slots.push(pos);
                    allocator.data[pos].take()
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(freed_value) = freed_value {
            freed_value.release_in_context(talk_context);

            TalkReleaseAction::Dropped
        } else {
            TalkReleaseAction::Retained
        }
    }
}

impl TalkInvertedClassAllocator {
    ///
    /// Creates an allocator with no values in it
    ///
    pub fn empty() -> Arc<Mutex<TalkInvertedClassAllocator>> {
        Arc::new(Mutex::new(TalkInvertedClassAllocator {
            next_priority:          0,
            responder_classes:      TalkSparseArray::empty(),
            respond_to_all:         vec![],
            respond_to_specific:    vec![],
            data:                   vec![],
            reference_counts:       vec![],
            free_slots:             vec![],
        }))
    }

    ///
    /// Callback when a reference is dropped
    ///
    fn on_dropped_reference(&mut self, reference: TalkReference, _talk_context: &TalkContext) {
        // TODO: deregister this reference from any `Inverted` subclass that 
    }

    ///
    /// Sends an inverted message to the known instances of the `Inverted` class that support the message type and have requested to receive it
    ///
    #[inline]
    fn send_inverted_message(allocator: &Arc<Mutex<Self>>, sender_reference: TalkOwned<TalkReference, &'_ TalkContext>, inverted_message: TalkOwned<TalkMessage, &'_ TalkContext>) -> TalkContinuation<'static> {
        let allocator           = Arc::clone(allocator);
        let sender_reference    = sender_reference.leak();
        let inverted_message    = inverted_message.leak();

        // There is a priority execution order: messages are received in reverse order of calling the `receiveFrom:` message.
        // 'unreceived' receiver targets are only called if the message has not been processed by any other receiver in this order

        // Possible targets (any individual object should only receive the message once even if matching multiple conditions):
        //      - objects registered for receiving directly from the sender which support the inverted message
        //      - objects registered for receiving all messages for any `Inverted` subclass that supports the message
        //      - TODO: objects registered for receiving messages in the local context
        //      - TODO: objects registered for receiving all messages from a specific class (or its subclass) - not sure if we have a way to get the superclass of a class at the 
        //              moment
        //
        // Some groups/priorities may have the 'unreceived' modifier and so need to only send if the message is 'unhandled'
        TalkContinuation::Soon(Box::new(move |talk_context, local_context| {
            let allocator   = allocator.lock().unwrap();
            let message_id  = inverted_message.signature_id();

            // For this to be an 'inverted' message, it must have some Inverted classes that respond to it
            if let Some(responder_classes) = allocator.responder_classes.get(message_id.into()) {
                // There are `Inverted` classes that can respond to this message
                let mut targets = vec![];

                // Note: when releasing targets, we're only told after the reference goes invalid. Right now the mutex in the runtime ensures that
                // this can't happen simultaneously with this continuation (TalkContext is meant to be Send but !Sync). There'd be a race condition
                // if this mutex didn't exist and it were possible for another thread to be releasing a target reference as this continuation is
                // starting to run (ie, this logic would need to be rethought if TalkContext needed to support multithreading).

                // Find all of the Inverted objects that always respond to this message
                for responder_class in responder_classes.iter() {
                    let responder_class_id = usize::from(*responder_class);

                    if responder_class_id < allocator.respond_to_all.len() {
                        targets.extend(allocator.respond_to_all[responder_class_id].iter().cloned());
                    }
                }

                // Find all of the Inverted objects that are specifically responding to this source
                let sender_class    = sender_reference.0;
                let sender_class_id = usize::from(sender_class);
                let sender_handle   = sender_reference.1;

                if sender_class_id < allocator.respond_to_specific.len() {
                    let sender_handle_id = usize::from(sender_handle);

                    if let Some(specific_responders) = allocator.respond_to_specific[sender_class_id].get(sender_handle_id) {
                        // These responders may or may not respond to this message, so we need to filter them to the responder classes
                        targets.extend(specific_responders.iter()
                            .filter(|(TalkReference(ref class_id, _) ,_)| responder_classes.contains(class_id))
                            .cloned());
                    }
                }

                // TODO: everything in the local context that might respond to this message
                // TODO: respond to specific class or subclass

                // Add the sender as a final parameter to the message (so it's released alongside it)
                let inverted_message = inverted_message.with_extra_parameter("invertedFrom:", sender_reference);

                match targets.len() {
                    0 => {
                          // No responders, so nothing to do (inverted messages don't error even if there are no responder)
                        TalkContinuation::soon(move |context| {
                            // Free the message as there's nothing to receive it
                            inverted_message.release_in_context(context);

                            // Result is nil
                            ().into()
                        })
                    }

                    1 => { 
                        // Send the message directly to the first target, no prioritisation or weeding to do
                        // TODO: how to inject the sender into the message argument list
                        todo!() 
                    }

                    _ => {
                        // If there are multiple targets, we need to make sure that each target is only in the list once, then we need to order by priority and deal with 'unreceived' groups too
                        todo!()
                    }
                }
            } else {
                // No `Inverted` class responds to this message
                TalkError::MessageNotSupported(inverted_message.signature_id()).into()
            }
        }))
    }
}

impl TalkInvertedClass {
    ///
    /// Adds the action to take when a reference is dropped
    ///
    /// The inverted class allocator tracks dropped references so it can remove references that are attached to particular objects
    ///
    fn add_drop_action(&self, allocator: Arc<Mutex<TalkInvertedClassAllocator>>, talk_context: &mut TalkContext) {
        talk_context.on_dropped_reference(move |reference, talk_context| {
            allocator.lock().unwrap().on_dropped_reference(reference, talk_context)
        })
    }
}

impl TalkClassDefinition for TalkInvertedClass {
    /// The type of the data stored by an object of this class
    type Data = TalkInverted;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkInvertedClassAllocator;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self, talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        let allocator = TalkInvertedClassAllocator::empty();

        self.add_drop_action(Arc::clone(&allocator), talk_context);

        allocator
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, class_id: TalkClass, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}

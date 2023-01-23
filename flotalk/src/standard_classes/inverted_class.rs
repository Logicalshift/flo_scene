use crate::context::*;
use crate::class::*;
use crate::continuation::*;
use crate::dispatch_table::*;
use crate::error::*;
use crate::message::*;
use crate::reference::*;
use crate::releasable::*;
use crate::sparse_array::*;
use crate::symbol::*;
use crate::value::*;
use crate::value_messages::*;

use super::later_class::*;
use super::script_class::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::collections::{HashSet};
use std::sync::*;

/// The 'Inverted' class, adds inverted-control messages
pub (crate) static INVERTED_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkInvertedClass));

/// A value representing the 'all' collection
pub (crate) static INVERTED_ALL: Lazy<TalkValue> = Lazy::new(|| TalkSymbol::new_unnamed().into());

/// A value representing the 'unhandled' result, used to keep a message being processed in the 'unreceived' state
pub (crate) static INVERTED_UNHANDLED: Lazy<TalkValue> = Lazy::new(|| TalkSymbol::new_unnamed().into());

/// A value representing the 'unhandled' result, used to keep a message being processed in the 'unreceived' state
pub (crate) static TALK_MSG_HANDLED: Lazy<TalkMessageSignatureId> = Lazy::new(|| "handled:".into());

/// Sending the 'unreceived' message to something turns it into the message 'unreceived: whatever', which receiveFrom: uses to set the appropriate flag
pub (crate) static INVERTED_UNRECEIVED_MSG: Lazy<TalkMessageSignatureId> = Lazy::new(|| "unreceived:".into());

/// Specifies an object to receive messages from
static TALK_MSG_RECEIVE_FROM: Lazy<TalkMessageSignatureId> = Lazy::new(|| "receiveFrom:".into());

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
/// the message). When there are multiple receivers, only one can generate a return value: the first receiver to call
/// `Inverted handled: <return value>` will set the return value of the stack of receivers as a whole.  
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
///     addInvertedMessage: #logDebug: withAction: [ :msg :sender :self | "... write debug message ..." ];
///     addInvertedMessage: #logError: withAction: [ :msg :sender :self | "... write error message ..." ].
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
/// ```
///
/// The `Inverted` instance that received the most recent `receiveFrom:` or `with:` call will receive the message with the 
/// highest priority for the cases where that matters.
///
/// An inverted instance message can return the value `Inverted unhandled` if it wants to indicate that it doesn't want 
/// the message to be considered as 'received' for lower priority handlers.
///
pub struct TalkInvertedClass;

///
/// Data stored for an instance of the inverted class
///
pub struct TalkInverted {

}

///
/// When a particular inverted message should be sent to a type (if it has not been received by another processor or always)
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub (crate) enum TalkProcessWhen {
    Always,
    Unreceived,    
}

///
/// The priority of an inverted receiver (higher priorities receive messages sooner)
///
#[derive(Copy, Clone, Debug)]
pub (crate) struct TalkPriority(usize, TalkProcessWhen);

///
/// Allocator for instances of the inverted class
///
pub struct TalkInvertedClassAllocator {
    /// The priority of the next receiveFrom: implementation that's added
    next_priority: usize,

    /// The classes in the current context that can respond to each type of message ID (expectation is for there to be usually just one class per message)
    responder_classes: TalkSparseArray<SmallVec<[TalkClass; 2]>>,

    /// The references that are registered as responders (have had receiveFrom: called on them)
    responder_instances: HashSet<TalkReference>,

    /// A Vec, indexed by responder class ID that contains the references of that class that want to respond to all
    /// Ie, for a given message to find out which 'all' responders are present, we look up the responder classes, then use them to look up each responder in here
    respond_to_all: Vec<Vec<(TalkReference, TalkPriority)>>,

    /// A Vec, indexed by *source* class ID that contains a sparse array of source data handles and the `Inverted` references that respond to them
    respond_to_specific: Vec<TalkSparseArray<Vec<(TalkReference, TalkPriority)>>>,

    /// The data store
    data: Vec<Option<TalkInverted>>,

    /// Reference counts for each allocated item in the data store (data is dropped when the count reaches 0)
    reference_counts: Vec<usize>,

    /// Items in the data array that have been freed and are available for reallocation
    free_slots: Vec<usize>,
}

impl TalkInvertedClass {
    ///
    /// Sends a message as an inverted message to anything that is listening for it in the current context
    ///
    #[inline]
    pub fn send_inverted_message(context: &TalkContext, source: TalkOwned<TalkReference, &'_ TalkContext>, inverted_message: TalkOwned<TalkMessage, &'_ TalkContext>) -> TalkContinuation<'static> {
        // Fetch the allocator for the inverted class from the context
        let callbacks = context.get_callbacks(*INVERTED_CLASS).unwrap();
        let allocator = callbacks.allocator.downcast_ref::<Arc<Mutex<TalkInvertedClassAllocator>>>()
            .map(|defn| Arc::clone(defn))
            .unwrap();

        // Ask the allocator to send the message
        TalkInvertedClassAllocator::send_inverted_message(allocator, source, inverted_message)
    }
}

impl TalkReleasable for TalkInverted {
    fn release_in_context(self, _context: &TalkContext) { }
}

impl TalkInvertedClassAllocator {
    ///
    /// Stores a value in this allocator and returns a handle to it
    ///
    #[inline]
    fn store(&mut self, value: TalkInverted) -> TalkDataHandle {
        if let Some(pos) = self.free_slots.pop() {
            self.data[pos]              = Some(value);
            self.reference_counts[pos]  = 1;

            TalkDataHandle(pos)
        } else {
            let pos = self.data.len();

            self.data.push(Some(value));
            self.reference_counts.push(1);

            TalkDataHandle(pos)
        }
    }
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
            responder_instances:    HashSet::new(),
            respond_to_all:         vec![],
            respond_to_specific:    vec![],
            data:                   vec![],
            reference_counts:       vec![],
            free_slots:             vec![],
        }))
    }

    ///
    /// Removes a responder from the list of responders
    ///
    fn drop_responder(&mut self, dropped_responder: TalkReference) {
        // Remove from the list of responders itself
        self.responder_instances.remove(&dropped_responder);

        // Remove from any respond_to_all receiver
        for all_responders in self.respond_to_all.iter_mut() {
            all_responders.retain(|(responder, _)| responder != &dropped_responder);
        }

        // Remove from any specific receiver
        for specific_class in self.respond_to_specific.iter_mut() {
            for (_, specific_data) in specific_class.iter_mut() {
                specific_data.retain(|(responder, _)| responder != &dropped_responder);
            }
        }
    }

    ///
    /// Callback when a reference is dropped
    ///
    fn on_dropped_reference(&mut self, reference: TalkReference, _talk_context: &TalkContext) {
        // Remove this reference from the list of values to respond to
        let class       = reference.0;
        let class_id    = usize::from(class);
        let handle      = reference.1;

        if class_id < self.respond_to_specific.len() {
            let handle_id = usize::from(handle);
            self.respond_to_specific[class_id].remove(handle_id);
        }

        // Remove from the responder list if needed
        if self.responder_instances.contains(&reference) {
            self.drop_responder(reference);
        }
    }

    ///
    /// Updates the expected return value after a block in an Inverted call has returned a value
    ///
    #[inline]
    fn result_value(latest_return: TalkValue, last_value: Option<TalkValue>, talk_context: &TalkContext) -> Option<TalkValue> {
        if last_value.is_some() {
            latest_return.release_in_context(talk_context);
            last_value
        } else if let TalkValue::Message(msg) = latest_return {
            // If the function returns the message `handled: xxx` then update the return value to be 'xxx'
            if let TalkMessage::WithArguments(message_id, mut args) = *msg {
                if message_id == *TALK_MSG_HANDLED {
                    Some(args[0].take())
                } else {
                    // Is a message, but wrong message signature
                    args.release_in_context(talk_context);
                    None
                }
            } else {
                // Is a unary message
                msg.release_in_context(talk_context);
                None
            }
        } else {
            latest_return.release_in_context(talk_context);
            None
        }
    }

    ///
    /// Creates a continuation that will call the specified set targets, in reverse order (by popping values) with the specified message
    ///
    /// Unreceived targets are only called if `message_is_received` is false
    ///
    fn call_targets(targets: Vec<(TalkReference, TalkPriority)>, message: TalkMessage, message_is_received: bool) -> TalkContinuation<'static> {
        let mut targets = targets;

        if targets.len() == 0 {
            // Ran out of targets to send to
            TalkContinuation::soon(|context| {
                message.release_in_context(context);
                ().into()
            })
        } else {
            // Get the target to send to
            let (target_ref, TalkPriority(_, when)) = targets.pop().unwrap();

            if when == TalkProcessWhen::Unreceived && message_is_received {
                // Target should not receive the message (marked as unreceived, and message is received)
                TalkContinuation::soon(move |context| {
                    target_ref.release(context);
                    Self::call_targets(targets, message, message_is_received)
                })
            } else {
                // Target should receive the message, then we should continue with the remaining targets (TODO: set message_is_received based on the return value)
                TalkContinuation::soon(move |context| {
                    let target_message = message.clone_in_context(context);
                    target_ref.send_message_in_context(target_message, context)
                        .and_then(move |result| {
                            // Message counts as received if it's already received or if the call did not return the 'unhandled' symbol
                            let was_received = message_is_received || result != *INVERTED_UNHANDLED;

                            Self::call_targets(targets, message, was_received)
                        })
                })
            }
        }
    }

    ///
    /// Sends an inverted message to the known instances of the `Inverted` class that support the message type and have requested to receive it
    ///
    fn send_inverted_message(allocator: Arc<Mutex<Self>>, sender_reference: TalkOwned<TalkReference, &'_ TalkContext>, inverted_message: TalkOwned<TalkMessage, &'_ TalkContext>) -> TalkContinuation<'static> {
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

                // Everything in the local context that might respond to this message
                if let Some(local_targets) = &local_context.inverted_targets {
                    for responder_class in responder_classes.iter() {
                        let responder_class_id = usize::from(*responder_class);

                        if let Some(responders) = local_targets.get(responder_class_id) {
                            targets.extend(responders.iter().cloned());
                        }
                    }
                }

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
                        let target = targets.pop().unwrap().0;

                        target.retain(talk_context);
                        target.send_message_in_context(inverted_message, talk_context)
                            .and_then_soon_if_ok(|val, talk_context| Self::result_value(val, None, talk_context).unwrap_or(TalkValue::Nil).into())
                    }

                    _ => {
                        // If there are multiple targets, we need to make sure that each target is only in the list once, then we need to order by priority and deal with 'unreceived' groups too
                        use std::cmp::{Ordering};

                        // Weeding out duplicate references: sort by reference
                        targets.sort_by(|(a_ref, TalkPriority(a_priority, _)), (b_ref, TalkPriority(b_priority, _))| {
                            let ref_ordering = a_ref.cmp(&b_ref);

                            if ref_ordering == Ordering::Equal {
                                // Order by priority, highest first
                                b_priority.cmp(a_priority)
                            } else {
                                // Order by reference
                                ref_ordering
                            }
                        });

                        // Weeding out duplicate references: fold duplicates into their highest-priority single entry
                        let mut idx = 1;
                        loop {
                            // All equal references will be in the same spot in this vec
                            let (a_ref, TalkPriority(_, a_when)) = &targets[idx-1];
                            let (b_ref, TalkPriority(_, b_when)) = &targets[idx];

                            // For cases where the references are the same, combine into a single entry
                            if a_ref == b_ref {
                                // Any 'Always' message will promote the 'when' to 'Always'
                                let when = match (a_when, b_when) {
                                    (TalkProcessWhen::Unreceived, TalkProcessWhen::Unreceived)  => TalkProcessWhen::Unreceived,
                                    _                                                   => TalkProcessWhen::Always,
                                };

                                // Update the 'process when' value
                                targets[idx-1].1.1 = when;

                                // Remove the next value as we want to send to any given reference once
                                targets.remove(idx);
                            } else {
                                // Move to the next value
                                idx += 1;
                            }

                            // Stop when we get to the end of the targets
                            if idx >= targets.len() {
                                break;
                            }
                        }

                        // Ready to send: now sort just by priority: higher priorities are put last so we can pop from the targets as we go
                        targets.sort_by(|(_, TalkPriority(a, _)), (_, TalkPriority(b, _))| { a.cmp(b) });

                        // Retain the target references (they get released as we send the messages)
                        for (target_ref, _) in targets.iter() {
                            target_ref.retain(talk_context);
                        }

                        // Begin calling the targets
                        Self::call_targets(targets, inverted_message, false)
                    }
                }
            } else {
                // No `Inverted` class responds to this message
                TalkError::MessageNotSupported(inverted_message.signature_id()).into()
            }
        }))
    }

    ///
    /// Sets it up so that 'target' will receive messages from 'source'
    ///
    fn receive_from_specific(&mut self, source: &TalkReference, target: &TalkReference, when: TalkProcessWhen) {
        let source_class    = usize::from(source.class());
        let source_handle   = usize::from(source.data_handle());
        let priority        = TalkPriority(self.next_priority, when);

        self.next_priority += 1;

        while self.respond_to_specific.len() <= source_class {
            self.respond_to_specific.push(TalkSparseArray::empty());
        }

        if let Some(responders) = self.respond_to_specific[source_class].get_mut(source_handle) {
            responders.push((target.clone(), priority));
        } else {
            self.respond_to_specific[source_class].insert(source_handle, vec![(target.clone(), priority)]);
        }
    }

    ///
    /// Sets it up so that 'target' will receive messages all possible sources
    ///
    fn receive_from_all(&mut self, target: &TalkReference, when: TalkProcessWhen) {
        let target_class    = usize::from(target.class());
        let priority        = TalkPriority(self.next_priority, when);

        self.next_priority += 1;

        while self.respond_to_all.len() <= target_class {
            self.respond_to_all.push(vec![]);
        }

        self.respond_to_all[target_class].push((target.clone(), priority));
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

    ///
    /// Given a continuation that generates a subclass, returns a continuation that adds the inverted instance messages such as 'receiveFrom:'
    ///
    fn declare_subclass_instance_messages(create_subclass: TalkContinuation<'static>) -> TalkContinuation<'static> {
        static TALK_MSG_WITH: Lazy<TalkMessageSignatureId>          = Lazy::new(|| "with:".into());
        static TALK_MSG_WITH_ASYNC: Lazy<TalkMessageSignatureId>    = Lazy::new(|| "withAsync:".into());

        create_subclass.and_then_soon_if_ok(|subclass_reference, talk_context| {
            // Modify the dispatch table
            let dispatch_table = talk_context.instance_dispatch_table(subclass_reference.try_as_reference().unwrap());

            dispatch_table.define_message(*TALK_MSG_RECEIVE_FROM,   |target, args, talk_context| Self::receive_from(target, args, talk_context));
            dispatch_table.define_message(*TALK_MSG_WITH,           |target, args, talk_context| Self::with(target, args, talk_context));
            dispatch_table.define_message(*TALK_MSG_WITH_ASYNC,     |target, args, talk_context| Self::with_async(target, args, talk_context));

            // Result is the subclass
            subclass_reference.into()
        })
    }

    ///
    /// Implements the 'addInvertedMessage:' message
    ///
    fn add_inverted_message(class_id: TalkOwned<TalkClass, &'_ TalkContext>, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _talk_context: &TalkContext) -> TalkContinuation<'static> {
        static TALK_MSG_ADD_INSTANCE_MESSAGE: Lazy<TalkMessageSignatureId>    = Lazy::new(|| ("addInstanceMessage:", "withAction:").into());

        // First argument should be a message selector
        let selector = match &args[0] {
            TalkValue::Selector(selector)   => *selector,
            _                               => { return TalkError::NotASelector.into(); }
        };

        // The selector we actually declare has an 'invertedFrom:' parameter
        let inverted_selector = selector.with_extra_parameter("invertedFrom:");

        // Leak the owned stuff so we can process it later on
        let class_id    = class_id.leak();
        let args        = args.leak();

        TalkContinuation::soon(move |talk_context| {
            // Mark the selector as inverted
            talk_context.add_inverted_message(selector);

            {
                // Fetch the allocator for the inverted class so we can add this selector
                let callbacks       = talk_context.get_callbacks(*INVERTED_CLASS).unwrap();
                let mut allocator   = callbacks.allocator.downcast_ref::<Arc<Mutex<TalkInvertedClassAllocator>>>().unwrap().lock().unwrap();

                // Declare this selector as handled by this class
                if let Some(responder_classes) = allocator.responder_classes.get_mut(selector.into()) {
                    // Add the class to the list for this selector
                    if !responder_classes.iter().any(|existing_class| existing_class == &class_id) {
                        responder_classes.push(class_id);
                    }
                } else {
                    // Create a new class
                    allocator.responder_classes.insert(selector.into(), smallvec![class_id])
                }
            }

            TalkContinuation::soon(move |talk_context| {
                // Change the argument to the inverted message selector
                let mut args    = args;
                args[0]         = TalkValue::Selector(inverted_selector);

                // Send the 'add instance message' request to the class
                class_id.send_message_in_context(TalkMessage::WithArguments(*TALK_MSG_ADD_INSTANCE_MESSAGE, args), talk_context)
            })
        })
    }

    ///
    /// Adds an instance for an inverted object to receive messages from 
    ///
    fn receive_from(target: TalkOwned<TalkReference, &'_ TalkContext>, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _talk_context: &TalkContext) -> TalkContinuation<'static> {
        let target  = target.leak();
        let args    = args.leak();

        TalkContinuation::soon(move |talk_context| {
            // Fetch the allocator
            let callbacks = talk_context.get_callbacks_mut(*INVERTED_CLASS);
            let allocator = callbacks.allocator.downcast_ref::<Arc<Mutex<TalkInvertedClassAllocator>>>()
                .map(|defn| Arc::clone(defn))
                .unwrap();

            // Determine when the item needs to be processed
            let (source, when) = if let TalkValue::Message(msg) = &args[0] {
                if msg.signature_id() == *INVERTED_UNRECEIVED_MSG {
                    match &**msg {
                        TalkMessage::WithArguments(_, msg_args) => (&msg_args[0], TalkProcessWhen::Unreceived),
                        _                                       => unreachable!()
                    }
                } else {
                    (&args[0], TalkProcessWhen::Always)
                }
            } else {
                (&args[0], TalkProcessWhen::Always)
            };

            // Register the message
            if let Ok(source) = source.try_as_reference() {
                // Source must be a reference, can't receive from value types
                let mut allocator = allocator.lock().unwrap();

                allocator.receive_from_specific(source, &target, when);
            } else if source == &*INVERTED_ALL {
                // Receive supported inverted messages from all objects
                let mut allocator = allocator.lock().unwrap();

                allocator.receive_from_all(&target, when);
            }

            // Release the arguments
            args.release_in_context(talk_context);
            target.release_in_context(talk_context);

            // Return result is nil
            ().into()
        })
    }

    ///
    /// Adds the 'target' class to the list of inverted receivers while a block passed in the arguments is executed
    ///
    fn with(target: TalkOwned<TalkReference, &'_ TalkContext>, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _talk_context: &TalkContext) -> TalkContinuation<'static> {
        // Target ends up retained in the local context
        let target = target.leak();

        // Only argument is the block
        let block = args.leak()[0].take();

        TalkContinuation::Soon(Box::new(move |talk_context, local_context| {
            // Fetch the allocator
            let callbacks = talk_context.get_callbacks_mut(*INVERTED_CLASS);
            let allocator = callbacks.allocator.downcast_ref::<Arc<Mutex<TalkInvertedClassAllocator>>>()
                .unwrap();

            // Get the priority from the allocator
            let priority = { 
                let mut allocator = allocator.lock().unwrap();
                let next_priority = allocator.next_priority;
                allocator.next_priority += 1;

                TalkPriority(next_priority, TalkProcessWhen::Always)
            };

            // Add the target to the local context
            let target_copy = target.clone();
            local_context.push_inverted_target(target, priority);

            // Send the 'value' message to the block with the updated context
            block.send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), talk_context)
                .and_then(move |result| {
                    TalkContinuation::Soon(Box::new(move |talk_context, local_context| {
                        // Remove the target from the local context again
                        local_context.pop_inverted_target(target_copy, talk_context);

                        // Return the same result
                        result.into()
                    }))
                })
        }))
    }

    ///
    /// As for 'with' except runs in background, and returns a Later
    ///
    fn with_async(target: TalkOwned<TalkReference, &'_ TalkContext>, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, talk_context: &TalkContext) -> TalkContinuation<'static> {
        static TALK_MSG_SENDER: Lazy<TalkMessageSignatureId>    = Lazy::new(|| "sender".into());
        static TALK_MSG_SETVALUE: Lazy<TalkMessageSignatureId>  = Lazy::new(|| "setValue:".into());

        // Target ends up retained in the local context
        let target = target.leak();

        // Only argument is the block
        let block = args.leak()[0].take();

        // Start by creating a 'Later' to call back
        LATER_CLASS.send_message_in_context(TalkMessage::Unary(*TALK_MSG_NEW), talk_context)
            .and_then(move |later_value| {
                TalkContinuation::Soon(Box::new(move |talk_context, _local_context| {
                    // Get the priority from the allocator
                    let callbacks = talk_context.get_callbacks_mut(*INVERTED_CLASS);
                    let allocator = callbacks.allocator.downcast_ref::<Arc<Mutex<TalkInvertedClassAllocator>>>()
                        .unwrap();

                    let priority = { 
                        let mut allocator = allocator.lock().unwrap();
                        let next_priority = allocator.next_priority;
                        allocator.next_priority += 1;

                        TalkPriority(next_priority, TalkProcessWhen::Always)
                    };

                    // Convert the 'later' value to a sender
                    let later_to_sender = later_value.clone_in_context(talk_context);

                    later_to_sender.send_message_in_context(TalkMessage::Unary(*TALK_MSG_SENDER), talk_context)
                        .and_then_soon(move |later_sender, talk_context| {
                            // Run the task in the background (TODO: clone the active local context)
                            talk_context.run_in_background(TalkContinuation::Soon(Box::new(move |talk_context, local_context| {
                                // Push the target (don't need to pop it as the whole context will be freed when this finishes)
                                local_context.push_inverted_target(target, priority);

                                // Evaluate the value and retrieve the result
                                block.send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), talk_context)
                                    .and_then_soon(move |result, talk_context| {
                                        // Send to the sender once done
                                        later_sender.send_message_in_context(TalkMessage::WithArguments(*TALK_MSG_SETVALUE, smallvec![result]), talk_context)
                                    })
                            })));

                            // Result is the 'later' value
                            later_value.into()
                        })
                }))
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
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, class_id: TalkClass, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        if message_id == *TALK_MSG_NEW {

            // Create a new 'Inverter' data object
            let new_value = TalkInverted { };

            // Store in the allocator
            let inverted_data_handle    = allocator.lock().unwrap().store(new_value);
            let inverted_reference      = TalkReference(class_id, inverted_data_handle);

            inverted_reference.into()

        } else {
            TalkError::MessageNotSupported(message_id).into()
        }
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _reference: TalkReference, _allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Generates default dispatch table for the class object for this class
    ///
    /// Messages are dispatched here ahead of the 'send_instance_message' callback (note in particular `respondsTo:` may need to be overridden)
    ///
    fn default_class_dispatch_table(&self) -> TalkMessageDispatchTable<TalkClass> {
        static TALK_MSG_SUBCLASS: Lazy<TalkMessageSignatureId>              = Lazy::new(|| "subclass".into());
        static TALK_MSG_ADD_INVERTED_MESSAGE: Lazy<TalkMessageSignatureId>  = Lazy::new(|| ("addInvertedMessage:", "withAction:").into());
        static TALK_MSG_UNHANDLED: Lazy<TalkMessageSignatureId>             = Lazy::new(|| "unhandled".into());

        TalkMessageDispatchTable::empty()
            .with_message(*TALK_MSG_SUBCLASS,               |class_id: TalkOwned<TalkClass, &'_ TalkContext>, _, _| Self::declare_subclass_instance_messages(TalkScriptClassClass::create_subclass(*class_id, vec![*TALK_MSG_NEW])))
            .with_message(*TALK_MSG_ADD_INVERTED_MESSAGE,   |class_id, args, talk_context|                          Self::add_inverted_message(class_id, args, talk_context))
            .with_message(*TALK_MSG_UNHANDLED,              |_, _, _|                                               TalkContinuation::Ready(INVERTED_UNHANDLED.clone()))
            .with_message(*TALK_MSG_HANDLED,                |_, args, _|                                            TalkValue::Message(Box::new(TalkMessage::WithArguments(*TALK_MSG_HANDLED, args.leak()))))
    }
}

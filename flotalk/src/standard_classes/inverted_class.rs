use crate::allocator::*;
use crate::context::*;
use crate::class::*;
use crate::continuation::*;
use crate::error::*;
use crate::message::*;
use crate::reference::*;
use crate::releasable::*;
use crate::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::sync::*;

///
/// The `Inverted` class provides a way to declare messages that are sent *from* an instance instead of *to* an instance.
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
pub struct TalkInvertedClass {

}

///
/// Data stored for an instance of the inverted class
///
pub struct TalkInverted {

}

///
/// Allocator for instances of the inverted class
///
pub struct TalkInvertedClassAllocator {
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
            data:               vec![],
            reference_counts:   vec![],
            free_slots:         vec![],
        }))
    }

    ///
    /// Callback when a reference is dropped
    ///
    pub fn on_drop_reference(&mut self, reference: TalkReference) {

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
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        TalkInvertedClassAllocator::empty()
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

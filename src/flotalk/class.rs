use super::context::*;
use super::continuation::*;
use super::message::*;
use super::reference::*;

use std::any::*;
use std::sync::*;
use std::cell::*;

lazy_static! {
    static ref NEXT_CLASS_ID: Mutex<usize>                                      = Mutex::new(0);
    static ref CLASS_CALLBACKS: Mutex<Vec<Option<&'static TalkClassCallbacks>>> = Mutex::new(vec![]);
}

thread_local! {
    static LOCAL_CLASS_CALLBACKS: RefCell<Vec<Option<&'static TalkClassCallbacks>>> = RefCell::new(vec![]);
}

///
/// Callbacks for addressing a TalkClass
///
pub (crate) struct TalkClassCallbacks {

}

///
/// A TalkClass is an identifier for a FloTalk class
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TalkClass(usize);

impl TalkClass {
    ///
    /// Creates a new class identifier
    ///
    fn new() -> TalkClass {
        let class_id = {
            let mut next_class_id   = NEXT_CLASS_ID.lock().unwrap();
            let class_id            = *next_class_id;
            *next_class_id          += 1;
            class_id
        };

        TalkClass(class_id)
    }
}

///
/// A class definition is a trait implemented by a FloTalk class
///
pub trait TalkClassDefinition : Send + Sync {
    /// The type of the data stored by an object of this class
    type Data: Send;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator: TalkClassAllocator<Data=Self::Data>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self) -> Self::Allocator;

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message: TalkMessage, allocator: &mut Self::Allocator) -> TalkContinuation;

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message: TalkMessage, reference: TalkReference, context: &TalkContext, target: &mut Self::Data) -> TalkContinuation;
}

///
/// A class allocator is used to manage the memory of a class
///
pub trait TalkClassAllocator : Send {
    /// The type of data stored for this class
    type Data: Send;

    ///
    /// Allocates data for an instance of this class. This data is allocated with a reference count of 1
    ///
    fn allocate(&mut self) -> TalkDataHandle;

    ///
    /// Retrieves a reference to the data attached to a handle (panics if the handle has been released)
    ///
    fn retrieve<'a>(&'a mut self, handle: TalkDataHandle) -> &'a mut Self::Data;

    ///
    /// Adds to the reference count for a data handle
    ///
    fn add_reference(&mut self, handle: TalkDataHandle);

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    fn remove_reference(&mut self, handle: TalkDataHandle);
}

impl TalkClass {
    ///
    /// Creates a TalkClass from a definition
    ///
    pub fn create(definition: impl TalkClassDefinition) -> TalkClass {
        // Create an identifier for this class
        let class           = TalkClass::new();
        let TalkClass(idx)  = class;

        // Create the class definition
        let class_callbacks = TalkClassCallbacks {

        };

        // Store as a static reference (classes live for the lifetime of the program)
        let class_callbacks     = Box::new(class_callbacks);
        let class_callbacks     = Box::leak(class_callbacks);
        let mut all_callbacks   = CLASS_CALLBACKS.lock().unwrap();

        while all_callbacks.len() <= idx {
            all_callbacks.push(None);
        }
        all_callbacks[idx] = Some(class_callbacks);

        // Return the definition we just created
        class
    }

    ///
    /// Looks up the callbacks for this class, 
    ///
    fn make_local_callbacks(&self) -> &'static TalkClassCallbacks {
        let TalkClass(idx) = *self;

        // Look up the callback in the global set
        let callback = (*CLASS_CALLBACKS.lock().unwrap())[idx].unwrap();

        // Store in the thread-local set so we can retrieve it more quickly in future
        LOCAL_CLASS_CALLBACKS.with(|local_class_callbacks| {
            let mut local_class_callbacks = local_class_callbacks.borrow_mut();

            while local_class_callbacks.len() <= idx {
                local_class_callbacks.push(None);
            }
            local_class_callbacks[idx] = Some(callback);
        });

        // Result is the callback we looked up
        callback
    }

    ///
    /// Retrieve the callbacks for this class
    ///
    #[inline]
    pub (crate) fn callbacks(&self) -> &'static TalkClassCallbacks {
        let TalkClass(idx)  = *self;
        let callback        = LOCAL_CLASS_CALLBACKS.with(|callbacks| {
            let callbacks = callbacks.borrow();

            if idx < callbacks.len() {
                callbacks[idx]
            } else {
                None
            }
        });

        if let Some(callback) = callback {
            callback
        } else {
            self.make_local_callbacks()
        }
    }

    ///
    /// Allocates an instance of this class in the specified context
    ///
    #[inline]
    pub fn allocate(&self, context: &mut TalkContext) -> TalkReference {
        todo!()
    }

    ///
    /// Sends a message to this class
    ///
    #[inline]
    pub fn send_class_message(&self, message: TalkMessage, context: &TalkContext) -> TalkContinuation {
        todo!()
    }
}

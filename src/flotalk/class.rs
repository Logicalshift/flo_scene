use super::continuation::*;
use super::message::*;
use super::reference::*;

use std::sync::*;

lazy_static! {
    static ref NEXT_CLASS_ID: Mutex<usize> = Mutex::new(0);
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
    pub fn new() -> TalkClass {
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
pub trait TalkClassDefinition {
    /// The type of the data stored by an object of this class
    type Data: Send;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator: TalkClassAllocator<Data=Self::Data>;

    ///
    /// Returns the ID for this class
    ///
    fn id(&self) -> TalkClass;

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
    fn send_instance_message(&self, message: TalkMessage, reference: TalkReference, target: &mut Self::Data) -> TalkContinuation;
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

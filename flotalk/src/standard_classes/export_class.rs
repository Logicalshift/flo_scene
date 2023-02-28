use crate::*;

use smallvec::*;

use std::sync::*;

///
/// The `Export` class is used to supply values that are used later by the `Import` class
///
/// This is used to specify which items are exported from a source file, such that they'll be returned by `Import item: 'val' from: 'File'`. It has a few usages:
///
/// * `Export value: <val> as: 'val'.` - export a value defined in the file
/// * `Export class: [ :Self | "..." ]` as: SampleClass.` - define a class
///
pub struct TalkExportClass;

///
/// The export allocator is used to define things that are exported by the `Export` class 
///
pub struct TalkExportAllocator {
    allocator: TalkStandardAllocator<()>,
}

impl TalkClassAllocator for TalkExportAllocator {
    type Data = ();

    fn retrieve<'a>(&'a mut self, handle: TalkDataHandle) -> &'a mut Self::Data {
        self.allocator.retrieve(handle)
    }

    fn retain(_allocator: &Arc<Mutex<Self>>, _handle: TalkDataHandle, _context: &TalkContext) {
        // No data is stored in the underlying allocator
    }

    fn release(_allocator: &Arc<Mutex<Self>>, _handle: TalkDataHandle, _context: &TalkContext) -> TalkReleaseAction {
        // No data is stored in the underlying allocator
        TalkReleaseAction::Dropped
    }
}

impl TalkClassDefinition for TalkExportClass {
    /// The type of the data stored by an object of this class
    type Data = ();

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkExportAllocator;

    ///
    /// Creates the allocator for this class in a particular context
    ///
    /// This is also an opportunity for a class to perform any other initialization it needs to do within a particular `TalkContext`
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        Arc::new(Mutex::new(TalkExportAllocator { 
            allocator:  TalkStandardAllocator::new(), 
        }))
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _reference: TalkReference, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}

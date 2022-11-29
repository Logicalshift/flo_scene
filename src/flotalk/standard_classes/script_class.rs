use crate::flotalk::allocator::*;
use crate::flotalk::class::*;
use crate::flotalk::context::*;
use crate::flotalk::continuation::*;
use crate::flotalk::error::*;
use crate::flotalk::message::*;
use crate::flotalk::reference::*;
use crate::flotalk::releasable::*;
use crate::flotalk::symbol_table::*;
use crate::flotalk::value::*;

use smallvec::*;

lazy_static! {
    /// `NewClass := Object subclass` will define a new class by subclassing Object. The new class will have no instance variables
    pub static ref TALK_MSG_SUBCLASS: TalkMessageSignatureId = "subclass".into();

    /// `NewClass := Object subclassWithInstanceVariables: #foo:bar:` will create a new class by subclassing object, with the instance variables 'foo' and 'bar'
    pub static ref TALK_MSG_SUBCLASS_WITH_INSTANCE_VARIABLES: TalkMessageSignatureId = "subclassWithInstanceVariables:".into();

    /// `NewClass addInstanceMessage: #instanceMessage: withAction: [:arg :self :super | arg + 1]` defines an instance message that works by sending a message to a block
    pub static ref TALK_MSG_ADD_INSTANCE_MESSAGE: TalkMessageSignatureId = ("addInstanceMessage:", "withAction:").into();

    /// `NewClass addClassMessage: #instanceMessage: withAction: [:arg :self :super | arg + 1]` defines a class message that works by sending a message to a block. Instance variables are bound to the block by this call.
    pub static ref TALK_MSG_ADD_CLASS_MESSAGE: TalkMessageSignatureId = ("addClassMessage:", "withAction:").into();
}

///
/// This class is a factory for other classes: it creates TalkScriptClass objects
///
pub struct TalkScriptClassClass {

}

///
/// This represents an instance of a talk script class
///
pub struct TalkScriptClass {
    /// The ID of the TalkCellBlockClass that this script class is associated with
    class_id: TalkClass,

    /// If this class has a superclass, the ID of that class
    superclass_id: Option<TalkClass>,

    /// The instance variables for this class
    instance_variables: TalkSymbolTable,
}

///
/// A cell block class is a class whose data type is a context cell block
///
pub struct TalkCellBlockClass;

///
/// Allocator that creates context cellblocks when requested
///
pub struct TalkCellBlockAllocator {
    /// Used as temporary storage for the 'retrieve' operation
    tmp_cell_block: TalkCellBlock
}

impl TalkReleasable for TalkScriptClass {
    fn release_in_context(self, _context: &TalkContext) {

    }
}

impl TalkClassDefinition for TalkScriptClassClass {
    /// The type of the data stored by an object of this class
    type Data = TalkScriptClass;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkStandardAllocator<TalkScriptClass>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self) -> Self::Allocator {
        TalkStandardAllocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, class_id: TalkClass, allocator: &mut Self::Allocator) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, reference: TalkReference, target: &mut Self::Data) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}

impl TalkClassDefinition for TalkCellBlockClass {
    /// The type of the data stored by an object of this class
    type Data = TalkCellBlock;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkCellBlockAllocator;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self) -> Self::Allocator {
        TalkCellBlockAllocator { tmp_cell_block: TalkCellBlock(0) }
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, class_id: TalkClass, allocator: &mut Self::Allocator) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, reference: TalkReference, target: &mut Self::Data) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}

impl TalkClassAllocator for TalkCellBlockAllocator {
    /// The type of data stored for this class
    type Data = TalkCellBlock;

    ///
    /// Retrieves a reference to the data attached to a handle (panics if the handle has been released)
    ///
    #[inline]
    fn retrieve<'a>(&'a mut self, handle: TalkDataHandle) -> &'a mut Self::Data {
        // Set to the temp value inside the allocator, and return that
        self.tmp_cell_block = TalkCellBlock(handle.0 as _);
        &mut self.tmp_cell_block
    }

    ///
    /// Adds to the reference count for a data handle
    ///
    #[inline]
    fn add_reference(&mut self, handle: TalkDataHandle, context: &TalkContext) {
        let cell_block = TalkCellBlock(handle.0 as _);
        context.retain_cell_block(cell_block);
    }

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    #[inline]
    fn remove_reference(&mut self, handle: TalkDataHandle, context: &TalkContext) {
        let cell_block = TalkCellBlock(handle.0 as _);
        context.release_cell_block(cell_block);
    }
}

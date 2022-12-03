use super::talk_message_handler::*;

use crate::flotalk::allocator::*;
use crate::flotalk::class::*;
use crate::flotalk::context::*;
use crate::flotalk::continuation::*;
use crate::flotalk::error::*;
use crate::flotalk::message::*;
use crate::flotalk::reference::*;
use crate::flotalk::releasable::*;
use crate::flotalk::symbol::*;
use crate::flotalk::symbol_table::*;
use crate::flotalk::sparse_array::*;
use crate::flotalk::value::*;
use crate::flotalk::value_messages::*;

use smallvec::*;

use std::sync::*;

lazy_static! {
    /// `NewClass := Object subclass` will define a new class by subclassing Object. The new class will have no instance variables
    pub static ref TALK_MSG_SUBCLASS: TalkMessageSignatureId = "subclass".into();

    /// `NewClass := Object subclassWithInstanceVariables: #foo:bar:` will create a new class by subclassing object, with the instance variables 'foo' and 'bar'
    pub static ref TALK_MSG_SUBCLASS_WITH_INSTANCE_VARIABLES: TalkMessageSignatureId = "subclassWithInstanceVariables:".into();

    /// `NewClass addInstanceMessage: #instanceMessage: withAction: [:arg :self :super | arg + 1]` defines an instance message that works by sending a message to a block
    pub static ref TALK_MSG_ADD_INSTANCE_MESSAGE: TalkMessageSignatureId = ("addInstanceMessage:", "withAction:").into();

    /// `NewClass addClassMessage: #instanceMessage: withAction: [:arg :self :super | arg + 1]` defines a class message that works by sending a message to a block. Instance variables are bound to the block by this call.
    pub static ref TALK_MSG_ADD_CLASS_MESSAGE: TalkMessageSignatureId = ("addClassMessage:", "withAction:").into();

    /// The 'class of classes', used for creating the scriptable classes like 'Object' and its subclasses
    pub static ref SCRIPT_CLASS_CLASS: TalkClass = TalkClass::create(TalkScriptClassClass);
}

///
/// This class is a factory for other classes: it creates TalkScriptClass objects
///
pub struct TalkScriptClassClass;

///
/// This represents an instance of a talk script class
///
pub struct TalkScriptClass {
    /// The ID of the TalkCellBlockClass that this script class is associated with
    class_id: TalkClass,

    /// The resources used by the instance messages (generally just the block)
    instance_message_resources: TalkSparseArray<SmallVec<[TalkValue; 2]>>,

    /// The resources used by the class messages (generally the block and the superclass)
    class_message_resources: TalkSparseArray<SmallVec<[TalkValue; 2]>>,

    /// If this class has a superclass, the ID of that class
    superclass_id: Option<TalkClass>,

    /// If the superclass is a script class, this is the reference to that class
    superclass_script_class: Option<TalkValue>,

    /// The instance variables for this class
    instance_variables: Arc<Mutex<TalkSymbolTable>>,
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
    fn release_in_context(mut self, context: &TalkContext) {
        // Release the superclass
        if let Some(superclass) = self.superclass_script_class.take() {
            superclass.remove_reference(context);
        }

        // Release all the resources used by the messages
        self.instance_message_resources.iter()
            .flat_map(|(_, references)| references.iter())
            .for_each(|reference| reference.remove_reference(context));
        self.class_message_resources.iter()
            .flat_map(|(_, references)| references.iter())
            .for_each(|reference| reference.remove_reference(context));
    }
}

impl TalkScriptClassClass {
    ///
    /// Creates a subclass of a superclass
    ///
    /// The parent_class reference is assumed to not be owned by this function
    ///
    fn subclass(our_class_id: TalkClass, parent_class: TalkReference, superclass: &TalkScriptClass) -> TalkContinuation<'static> {
        // Read the superclass ID from the class data
        let new_superclass_id = superclass.class_id;

        // Need a few copies of the reference
        let parent_class_1 = parent_class;
        let parent_class_2 = parent_class_1.clone();

        // Create a new script class by sending a message to ourselves
        TalkContinuation::soon(move |context| {
            // Retain the parent class (need to do this 'soon' as it may be released otherwise)
            parent_class_1.add_reference(context);

            // The 'new' message should generate a new script class reference
            our_class_id.send_message_in_context(TalkMessage::Unary(*TALK_MSG_NEW), context)
        }).and_then(move |new_class_reference| {
            // Set the superclass for this class

            // TODO: if read_value errors, it will leak the parent class
            TalkContinuation::read_value::<Self, _>(new_class_reference.clone(), move |script_class| {
                // The script_class will release the superclass when it's released (matching the add_reference above)
                script_class.superclass_id              = Some(new_superclass_id);
                script_class.superclass_script_class    = Some(TalkValue::Reference(parent_class_2));

                // As this is a subclass, location 0 is a pointer to the superclass
                script_class.instance_variables.lock().unwrap().define_symbol(*TALK_SUPER);

                new_class_reference
            })
        })
    }

    ///
    /// Creates a subclass of a superclass and declares a block of instance variables
    ///
    /// The parent_class reference is assumed to not be owned by this function
    ///
    fn subclass_with_instance_variables(our_class_id: TalkClass, parent_class: TalkReference, superclass: &TalkScriptClass, variables: TalkMessageSignature) -> TalkContinuation<'static> {
        Self::subclass(our_class_id, parent_class, superclass).and_then(move |new_class_reference| {
            // Set the symbol table for this class (the symbols in the message signature become the instance variables)
            TalkContinuation::read_value::<Self, _>(new_class_reference.clone(), move |script_class| {
                let mut instance_variables = script_class.instance_variables.lock().unwrap();

                match variables {
                    TalkMessageSignature::Unary(symbol)     => { instance_variables.define_symbol(symbol); },
                    TalkMessageSignature::Arguments(args)   => { args.into_iter().for_each(|symbol| { instance_variables.define_symbol(symbol.keyword_to_symbol()); }); },
                }

                new_class_reference
            })
        })
    }
}

impl TalkScriptClass {
    ///
    /// Adds a class message that calls the specified block
    ///
    fn add_class_message(&mut self, selector: TalkMessageSignature, block: TalkOwned<'_, TalkValue>) -> TalkContinuation<'static> {
        let cell_class_id   = self.class_id;
        let context         = block.context();
        let message_handler = block.read_data_in_context::<TalkClassMessageHandler>(context);

        if let Some(message_handler) = message_handler {
            // Fetch the superclass and retain it
            let superclass_value = self.superclass_script_class.as_ref().map(|superclass| superclass.clone_in_context(block.context()));

            // Keep the block associated with this class
            let mut resources   = smallvec![];
            let message_id      = usize::from(TalkMessageSignatureId::from(&selector));

            if let Some(old_resources) = self.instance_message_resources.remove(message_id) {
                // Clean up any old message that might be stored here
                old_resources.into_iter().for_each(|reference| reference.remove_reference(block.context()));
            }

            // Also retain the superclass if it's present
            if let Some(superclass_value) = &superclass_value {
                resources.push(superclass_value.clone());
            }

            resources.push(block.leak());
            self.class_message_resources.insert(message_id, resources);

            // Add to the dispatch table for the cell class in the current context
            TalkContinuation::soon(move |context| {
                (message_handler.define_in_dispatch_table)(&mut context.get_callbacks_mut(cell_class_id).class_dispatch_table, selector.into(), superclass_value.clone());

                TalkValue::Nil.into()
            })
        } else {
            // Unexpected class
            TalkError::ExpectedBlockType.into()
        }
    }

    ///
    /// Adds an instance message that calls the specified block (which is rebound to the instance variables)
    ///
    fn add_instance_message(&mut self, selector: TalkMessageSignature, block: TalkOwned<'_, TalkValue>, instance_variables: Arc<Mutex<TalkSymbolTable>>) -> TalkContinuation<'static> {
        let cell_class_id   = self.class_id;
        let context         = block.context();
        let message_handler = block.read_data_in_context::<TalkInstanceMessageHandler>(context);

        if let Some(message_handler) = message_handler {
            // Keep the block associated with this class
            let message_id = usize::from(TalkMessageSignatureId::from(&selector));

            if let Some(old_resources) = self.instance_message_resources.remove(message_id) {
                // Clean up any old message that might be stored here
                old_resources.into_iter().for_each(|reference| reference.remove_reference(block.context()));
            }

            self.instance_message_resources.insert(message_id, smallvec![block.leak()]);

            // Add to the dispatch table for the cell class in the current context
            TalkContinuation::soon(move |context| {
                (message_handler.define_in_dispatch_table)(&mut context.get_callbacks_mut(cell_class_id).dispatch_table, selector.into(), instance_variables);

                TalkValue::Nil.into()
            })
        } else {
            // Unexpected class
            TalkError::ExpectedBlockType.into()
        }
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
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, class_id: TalkClass, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        if message_id == *TALK_MSG_NEW {

            let allocator = Arc::clone(allocator);

            TalkContinuation::soon(move |talk_context| {
                // Create a new cell block class with no superclass
                let cell_block_class = talk_context.empty_cell_block_class();

                // Define in a script class object (which is empty for now)
                let script_class = TalkScriptClass {
                    class_id:                   cell_block_class,
                    superclass_id:              None,
                    superclass_script_class:    None,
                    instance_variables:         Arc::new(Mutex::new(TalkSymbolTable::empty())),
                    instance_message_resources: TalkSparseArray::empty(),
                    class_message_resources:    TalkSparseArray::empty(),
                };

                // Store the class using the allocator
                let script_class = allocator.lock().unwrap().store(script_class);

                // Register the class with the context
                let script_class = TalkReference(class_id, script_class);
                talk_context.declare_cell_block_class(script_class.clone_in_context(talk_context), cell_block_class);

                // Result is a reference to the script class (this acts as the class object instead of a TalkClass object)
                script_class.into()
            })

        } else {

            TalkError::MessageNotSupported(message_id).into()
        }
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, reference: TalkReference, target: &mut Self::Data) -> TalkContinuation<'static> {
        // Try to send to the dispatch table for the cell class
        let context         = args.context();
        let class_id        = target.class_id;
        let dispatch_table  = context.get_callbacks(class_id).map(|callbacks| &callbacks.class_dispatch_table);

        // TODO: also check the superclass for class methods
        if let Some(dispatch_table) = dispatch_table {
            if dispatch_table.responds_to(message_id) {
                // If the class dispatch table responds to the message, forward it there instead
                let message = if args.len() == 0 {
                    TalkMessage::Unary(message_id)
                } else {
                    TalkMessage::WithArguments(message_id, args.leak())
                };

                return TalkContinuation::soon(move |context| {
                    let dispatch_table  = &context.get_callbacks(class_id).unwrap().class_dispatch_table;
                    dispatch_table.send_message((), message, context)
                })
            }
        }


        // Predefined messages
        if message_id == *TALK_MSG_SUBCLASS {

            // Create a subclass of this class
            Self::subclass(reference.class(), reference, target)

        } else if message_id == *TALK_MSG_SUBCLASS_WITH_INSTANCE_VARIABLES {

            // Create a subclass of this class with different instance variables
            match args[0] {
                TalkValue::Selector(args)   => Self::subclass_with_instance_variables(reference.class(), reference, target, args.to_signature()),
                _                           => TalkError::NotASelector.into(),
            }

        } else if message_id == *TALK_MSG_ADD_INSTANCE_MESSAGE {

            // Add an instance message for this class
            let mut args = args;
            match args[0] {
                TalkValue::Selector(selector)   => target.add_instance_message(selector.to_signature(), TalkOwned::new(args[1].take(), args.context()), Arc::clone(&target.instance_variables)),
                _                               => TalkError::NotASelector.into(),
            }

        } else if message_id == *TALK_MSG_ADD_CLASS_MESSAGE {

            // Add a message to the class messages for this class
            let mut args = args;
            match args[0] {
                TalkValue::Selector(selector)   => target.add_class_message(selector.to_signature(), TalkOwned::new(args[1].take(), args.context())),
                _                               => TalkError::NotASelector.into(),
            }

        } else if message_id == *TALK_MSG_SUPERCLASS {

            // Retrieve the superclass for this clas
            if let Some(superclass) = &target.superclass_script_class {
                let superclass = superclass.clone();
                TalkContinuation::soon(move |context| superclass.clone_in_context(context).into())
            } else {
                TalkValue::Nil.into()
            }

        } else if message_id == *TALK_MSG_NEW {

            // Create a new instance of this class (with empty instance variables)
            let instance_size   = target.instance_variables.lock().unwrap().len();
            let class_id        = target.class_id;

            if let Some(superclass) = &target.superclass_script_class {
                let superclass = superclass.clone();
                TalkContinuation::soon(move |context| {
                    // Send the 'new' message to the superclass
                    superclass.add_reference(context);
                    superclass.send_message_in_context(TalkMessage::Unary(*TALK_MSG_NEW), context)
                }).and_then_soon(move |superclass, context| {
                    match superclass {
                        TalkValue::Error(err)   => err.into(),
                        _                       => {
                            // Allocate space for this instance
                            let cell_block = context.allocate_cell_block(instance_size);

                            // The first value is always a reference to the superclass
                            context.cell_block_mut(cell_block)[0] = superclass;

                            // The result is a reference to the newly created object (cell block classes use their cell block as the data handle)
                            let handle      = TalkDataHandle(cell_block.0 as _);
                            let reference   = TalkReference(class_id, handle);

                            TalkValue::Reference(reference).into()
                        }
                    }
                })
            } else {
                TalkContinuation::soon(move |context| {
                    // Allocate space for this instance
                    let cell_block = context.allocate_cell_block(instance_size);

                    // The result is a reference to the newly created object (cell block classes use their cell block as the data handle)
                    let handle      = TalkDataHandle(cell_block.0 as _);
                    let reference   = TalkReference(class_id, handle);

                    TalkValue::Reference(reference).into()
                })
            }

        } else {

            TalkError::MessageNotSupported(message_id).into()
        }
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
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, _reference: TalkReference, _target: &mut Self::Data) -> TalkContinuation<'static> {
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
    fn add_reference(_allocator: &Arc<Mutex<Self>>, handle: TalkDataHandle, context: &TalkContext) {
        let cell_block = TalkCellBlock(handle.0 as _);
        context.retain_cell_block(cell_block);
    }

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    #[inline]
    fn remove_reference(_allocator: &Arc<Mutex<Self>>, handle: TalkDataHandle, context: &TalkContext) {
        let cell_block = TalkCellBlock(handle.0 as _);
        context.release_cell_block(cell_block);
    }
}

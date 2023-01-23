use super::talk_message_handler::*;

use crate::class::*;
use crate::context::*;
use crate::continuation::*;
use crate::error::*;
use crate::message::*;
use crate::reference::*;
use crate::releasable::*;
use crate::symbol::*;
use crate::symbol_table::*;
use crate::sparse_array::*;
use crate::value::*;
use crate::value_messages::*;
use crate::standard_classes::class_class::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::sync::*;

/// `newValue := Object new` will send the 'init' instance message before returning its result
pub static TALK_MSG_INIT: Lazy<TalkMessageSignatureId> = Lazy::new(|| "init".into());

/// When `new` is called on a script class, the superclass is created by calling `newSuperclass` (this allows for customizing how superclasses are created)
pub static TALK_MSG_NEWSUPERCLASS: Lazy<TalkMessageSignatureId> = Lazy::new(|| "newSuperclass".into());

/// `NewClass := Object subclass` will define a new class by subclassing Object. The new class will have no instance variables
pub static TALK_MSG_SUBCLASS: Lazy<TalkMessageSignatureId> = Lazy::new(|| "subclass".into());

/// `NewClass := Object subclassWithInstanceVariables: #foo:bar:` will create a new class by subclassing object, with the instance variables 'foo' and 'bar'
pub static TALK_MSG_SUBCLASS_WITH_INSTANCE_VARIABLES: Lazy<TalkMessageSignatureId> = Lazy::new(|| "subclassWithInstanceVariables:".into());

/// `NewClass addInstanceMessage: #instanceMessage: withAction: [:arg :self :super | arg + 1]` defines an instance message that works by sending a message to a block
pub static TALK_MSG_ADD_INSTANCE_MESSAGE: Lazy<TalkMessageSignatureId> = Lazy::new(|| ("addInstanceMessage:", "withAction:").into());

/// `NewClass addClassMessage: #instanceMessage: withAction: [:arg :self :super | arg + 1]` defines a class message that works by sending a message to a block. Instance variables are bound to the block by this call.
pub static TALK_MSG_ADD_CLASS_MESSAGE: Lazy<TalkMessageSignatureId> = Lazy::new(|| ("addClassMessage:", "withAction:").into());

/// The 'class of classes', used for creating the scriptable classes like 'Object' and its subclasses
pub (crate) static SCRIPT_CLASS_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkScriptClassClass));

///
/// This class is a factory for other classes: it creates TalkScriptClass objects
///
/// This not typically used directly but indirectly to create the base 'Object' class, or when subclassing another
/// class (eg: a custom class might call `TalkScriptClass::create_subclass()`). Sending a 'new' message to this
/// creates a new empty script class.
///
pub struct TalkScriptClassClass;

///
/// This represents an instance of a talk script class
///
/// There's typically an instance of this referenced by the `Object` symbol:
///
/// * `Object new` - create a new object
/// * `Subclass := Object subclass` - create a new subclass from Object
/// * `Subsubclass := Subclass subclass` - ... and so on
/// * `Subclass := Object subclassWithInstanceVariables: #foo:` - create a subclass with the instance variable 'foo' declared
/// * `Subclass addClassMessage: #classMessage: withAction: [ :arg :Self | "..." ]` - add a class message to a subclass
/// * `Subclass addInstanceMessage: #instanceMessage: withAction: [ :arg :self | "..." ]` - add an instance message (instance variables will be available from within the block, as will the `super` variable, which is implemented as another instance variable)
/// * `Subclass addClassMessage: #newSuperclass withAction: [ Object new ]` - the `newSuperclass` class message can be used to change how the `super` variable is initially set up
/// * `Subclass addInstanceMessage: #init withAction: [ :self | "..." ]` - the `init` message can be used to change how the new subclass sets up after being created
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
/// Script classes are allocated as class IDs, which we use in the references
///
pub struct TalkScriptClassAllocator {
    /// The classes that have been allocated by this object, indexed by class ID
    classes: TalkSparseArray<TalkScriptClass>,

    /// The reference counts for each class, indexed by class ID
    reference_counts: TalkSparseArray<usize>,
}

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
            superclass.release(context);
        }

        // Release all the resources used by the messages
        self.instance_message_resources.iter()
            .flat_map(|(_, references)| references.iter())
            .for_each(|reference| reference.release(context));
        self.class_message_resources.iter()
            .flat_map(|(_, references)| references.iter())
            .for_each(|reference| reference.release(context));
    }
}

impl TalkScriptClassClass {
    ///
    /// Creates a subclass of a class that is not itself a script class
    ///
    /// This provides a way to implement the `subclass` and `subclassWithInstanceVariables:` messages for any class. num_cells indicates the number of cells
    /// to allocate to objects in the resulting subclass (indexed from 1 in the cell block allocated to each instance).
    ///
    pub fn create_subclass(superclass: TalkClass, num_cells: usize, constructor_messages: Vec<TalkMessageSignatureId>) -> TalkContinuation<'static> {
        TalkContinuation::soon(move |context| {
            // Generate a new script class reference
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::Unary(*TALK_MSG_NEW), context)
        }).and_then_if_ok(move |new_class_reference| {
            let new_class_reference = Self::class_reference_to_script_reference(new_class_reference.try_as_reference().unwrap()).into();

            // Set the superclass for the new class
            TalkContinuation::read_value::<Self, _>(new_class_reference, move |script_class, _| {
                // Set the superclass
                script_class.superclass_id = Some(superclass);

                // No script superclass
                script_class.superclass_script_class = None;

                // As this is a subclass, location 0 is a pointer to the superclass
                script_class.instance_variables.lock().unwrap().define_symbol(*TALK_SUPER);
                let _first_cell = script_class.instance_variables.lock().unwrap().reserve_cells(num_cells);
                debug_assert!(_first_cell == 1);

                // Fetch the class ID of the subclass (this will always be a cell class)
                let cell_class_id       = script_class.class_id;
                let instance_variables  = Arc::clone(&script_class.instance_variables);

                TalkContinuation::soon(move |context| {
                    // Set the superclass in the context
                    context.set_superclass(cell_class_id, superclass);

                    // Set the class dispatch table to call the superclass for an unsupported message
                    let instance_dispatch_table = &mut context.get_callbacks_mut(cell_class_id).dispatch_table;

                    instance_dispatch_table.define_not_supported(move |cell_block_reference, msg, args, context| {
                        // As we know that the 'cell block' reference is has cell block handle, we can convert the data handle directly to a CellBlock
                        let cell_block = TalkCellBlock(cell_block_reference.1.0 as _);

                        // For classes with a superclass, the first value in the cell block is the superclass reference
                        let superclass_ref = &context.cell_block(cell_block)[0];
                        let superclass_ref = superclass_ref.clone_in_context(context);

                        if args.len() == 0 {
                            superclass_ref.send_message_in_context(TalkMessage::Unary(msg), context)
                        } else {
                            superclass_ref.send_message_in_context(TalkMessage::WithArguments(msg, args.leak()), context)
                        }
                    });

                    // Set up the constructor messages for the new class
                    let class_dispatch_table = &mut context.get_callbacks_mut(cell_class_id).class_dispatch_table;
                    constructor_messages.into_iter()
                        .for_each(|constructor_message_id| {
                            let cell_class_id = cell_class_id;

                            class_dispatch_table.define_message(constructor_message_id, move |_, args, context| {
                                // Construct the superclass
                                if args.len() == 0 {
                                    superclass.send_message_in_context(TalkMessage::Unary(constructor_message_id), context)
                                } else {
                                    superclass.send_message_in_context(TalkMessage::WithArguments(constructor_message_id, args.leak()), context)
                                }.and_then_soon_if_ok(move |new_class_value, context| {
                                    // Allocate space for this instance
                                    let cell_block = context.allocate_cell_block(1);

                                    // The first value is always a reference to the superclass
                                    context.cell_block_mut(cell_block)[0] = new_class_value;

                                    // The result is a reference to the newly created object (cell block classes use their cell block as the data handle)
                                    let handle      = TalkDataHandle(cell_block.0 as _);
                                    let reference   = TalkReference(cell_class_id, handle);

                                    // Call the 'init' message and return the reference
                                    reference.clone_in_context(context).send_message_in_context(TalkMessage::Unary(*TALK_MSG_INIT), context)
                                        .and_then(|_| {
                                            TalkValue::Reference(reference).into()
                                        })
                                })
                            })
                        });

                    Self::declare_class_messages(context, cell_class_id, Some(superclass), instance_variables);

                    // Final result is the new class reference
                    cell_class_id.class_object_in_context(context).into()
                })
            })
        })
    }

    ///
    /// Creates a subclass of a superclass
    ///
    /// The parent_class reference is assumed to not be owned by this function
    ///
    fn subclass(script_class_class: TalkClass, parent_class: TalkReference, superclass: &TalkScriptClass) -> TalkContinuation<'static> {
        // Read the superclass ID from the class data
        let new_superclass_id = superclass.class_id;

        // Need a few copies of the reference
        let parent_class_1 = parent_class;
        let parent_class_2 = parent_class_1.clone();

        // Create a new script class by sending a message to ourselves
        TalkContinuation::soon(move |context| {
            // Retain the parent class (need to do this 'soon' as it may be released otherwise)
            parent_class_1.retain(context);

            // The 'new' message should generate a new script class reference
            script_class_class.send_message_in_context(TalkMessage::Unary(*TALK_MSG_NEW), context)
        }).and_then(move |new_class_reference| {
            // Set the superclass for this class
            let new_class_reference = Self::class_reference_to_script_reference(new_class_reference.try_as_reference().unwrap());
            let new_class_reference = TalkValue::Reference(new_class_reference);

            // TODO: if read_value errors, it will leak the parent class
            TalkContinuation::read_value::<Self, _>(new_class_reference.clone(), move |script_class, _| {
                // The script_class will release the superclass when it's released (matching the retain above)
                script_class.superclass_id              = Some(new_superclass_id);
                script_class.superclass_script_class    = Some(TalkValue::Reference(parent_class_2));

                // As this is a subclass, location 0 is a pointer to the superclass
                script_class.instance_variables.lock().unwrap().define_symbol(*TALK_SUPER);

                // Set the superclass in the context
                let cell_class_id = script_class.class_id;
                TalkContinuation::soon(move |context| {
                    context.set_superclass(cell_class_id, new_superclass_id);
                    new_class_reference.into()
                })
            })
        }).and_then(move |new_class_reference| {
            // Call the superclass from the new class
            TalkContinuation::read_value::<Self, _>(new_class_reference.clone(), move |script_class, _| {
                let cell_class_id       = script_class.class_id;
                let instance_variables  = Arc::clone(&script_class.instance_variables);

                TalkContinuation::soon(move |context| {
                    // Declare the standard messages on the class object
                    Self::declare_class_messages(context, cell_class_id, Some(new_superclass_id), instance_variables);

                    // Set the class dispatch table to call the superclass for an unsupported message
                    let instance_dispatch_table = &mut context.get_callbacks_mut(cell_class_id).dispatch_table;

                    instance_dispatch_table.define_not_supported(move |cell_block_reference, msg, args, context| {
                        // As we know that the 'cell block' reference is has cell block handle, we can convert the data handle directly to a CellBlock
                        let cell_block = TalkCellBlock(cell_block_reference.1.0 as _);

                        // For classes with a superclass, the first value in the cell block is the superclass reference
                        let superclass_ref = &context.cell_block(cell_block)[0];
                        let superclass_ref = superclass_ref.clone_in_context(context);

                        if args.len() == 0 {
                            superclass_ref.send_message_in_context(TalkMessage::Unary(msg), context)
                        } else {
                            superclass_ref.send_message_in_context(TalkMessage::WithArguments(msg, args.leak()), context)
                        }
                    });

                    cell_class_id.class_object_in_context(context).into()
                })
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
            // The new class should be a TalkClass reference
            let script_class_reference = Self::class_reference_to_script_reference(new_class_reference.try_as_reference().unwrap());
            let script_class_reference = TalkValue::Reference(script_class_reference);

            // Set the symbol table for this class (the symbols in the message signature become the instance variables)
            TalkContinuation::read_value::<Self, _>(script_class_reference.clone(), move |script_class, _| {
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

impl TalkScriptClassClass {
    ///
    /// Converts a cell block class created by the script class to a TalkReference to the underlying ScriptClass data
    ///
    #[inline]
    fn cell_class_reference(class: TalkClass) -> TalkReference {
        TalkReference(*SCRIPT_CLASS_CLASS, TalkDataHandle(class.into()))
    }

    ///
    /// Converts a cell block class created by the script class to a TalkReference to the underlying ScriptClass data
    ///
    #[inline]
    fn class_reference_to_script_reference(class: &TalkReference) -> TalkReference {
        // Both CLASS_CLASS and SCRIPT_CLASS_CLASS use the TalkClass as the data handle (see the allocators for both)
        debug_assert!(class.class() == *CLASS_CLASS || class.class() == *SCRIPT_CLASS_CLASS);

        TalkReference(*SCRIPT_CLASS_CLASS, class.data_handle())
    }

    ///
    /// Declares the standard class messages on a new subclass
    ///
    fn declare_class_messages(context: &mut TalkContext, cell_class_id: TalkClass, superclass_id: Option<TalkClass>, instance_variables: Arc<Mutex<TalkSymbolTable>>) {
        let class_dispatch_table = &mut context.get_callbacks_mut(cell_class_id).class_dispatch_table;

        // Declare the 'addInstanceMessage:' type (caution: we assume the class reference stays alive)
        class_dispatch_table.define_message(*TALK_MSG_ADD_INSTANCE_MESSAGE,             Self::define_add_instance_message(cell_class_id, Arc::clone(&instance_variables)));
        class_dispatch_table.define_message(*TALK_MSG_ADD_CLASS_MESSAGE,                Self::define_add_class_message(cell_class_id));
        class_dispatch_table.define_message(*TALK_MSG_SUBCLASS,                         Self::define_subclass(cell_class_id));
        class_dispatch_table.define_message(*TALK_MSG_SUBCLASS_WITH_INSTANCE_VARIABLES, Self::define_subclass_with_instance_variables(cell_class_id));
        class_dispatch_table.define_message(*TALK_MSG_SUPERCLASS,                       Self::define_superclass(cell_class_id));
        class_dispatch_table.define_message(*TALK_MSG_NEWSUPERCLASS,                    Self::define_new_superclass(cell_class_id));
        class_dispatch_table.define_message(*TALK_MSG_NEW,                              Self::define_new(cell_class_id));

        // Send undispatched messages to the superclass if possible
        if let Some(superclass_id) = superclass_id {
            class_dispatch_table.define_not_supported(move |source_class, message_id, args, context| {
                superclass_id.send_message_in_context_from_subclass(*source_class, TalkMessage::from_signature(message_id, args.leak()), context)
            })
        } else {
            class_dispatch_table.define_not_supported(|_, id, _, _| TalkError::MessageNotSupported(id).into())
        }
    }

    ///
    /// Generates the function to use for the `addInstanceMessage:` message
    ///
    fn define_add_instance_message(cell_class_id: TalkClass, instance_variables: Arc<Mutex<TalkSymbolTable>>) -> impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TalkClass, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static> {
        move |_, args, _| {
            let mut args            = args.leak();
            let instance_variables  = Arc::clone(&instance_variables);

            TalkContinuation::read_value::<TalkScriptClassClass, _>(Self::cell_class_reference(cell_class_id).into(), move |script_class, talk_context| {
                debug_assert!(script_class.class_id == cell_class_id);

                // Add an instance message for this class
                match args[0] {
                    TalkValue::Selector(selector)   => script_class.add_instance_message(selector.to_signature(), TalkOwned::new(args[1].take(), talk_context), Arc::clone(&instance_variables)),
                    _                               => { args.release_in_context(talk_context); TalkError::NotASelector.into() },
                }
            })
        }
    }

    ///
    /// Generates the function to use for the 'addClassMessage:` message
    ///
    fn define_add_class_message(cell_class_id: TalkClass) -> impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TalkClass, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static> {
        move |_, args, _| {
            let mut args = args.leak();

            TalkContinuation::read_value::<TalkScriptClassClass, _>(Self::cell_class_reference(cell_class_id).into(), move |script_class, talk_context| {
                debug_assert!(script_class.class_id == cell_class_id);

                // Add a message to the class messages for this class
                match args[0] {
                    TalkValue::Selector(selector)   => script_class.add_class_message(selector.to_signature(), TalkOwned::new(args[1].take(), talk_context)),
                    _                               => { args.release_in_context(talk_context); TalkError::NotASelector.into() },
                }
            })
        }
    }

    ///
    /// Generates the function to use for the `subclass` message
    ///
    fn define_subclass(cell_class_id: TalkClass) -> impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TalkClass, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static> {
        move |_, _, _| {
            TalkContinuation::read_value::<TalkScriptClassClass, _>(Self::cell_class_reference(cell_class_id).into(), move |script_class, _| {
                debug_assert!(script_class.class_id == cell_class_id);

                TalkScriptClassClass::subclass(*SCRIPT_CLASS_CLASS, Self::cell_class_reference(cell_class_id), script_class)
            })
        }
    }

    ///
    /// Generates the function to use for the `subclassWithInstanceVariables:` message
    ///
    fn define_subclass_with_instance_variables(cell_class_id: TalkClass) -> impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TalkClass, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static> {
        move |_, args, _| {
            let args = args.leak();

            TalkContinuation::read_value::<TalkScriptClassClass, _>(Self::cell_class_reference(cell_class_id).into(), move |script_class, talk_context| {
                debug_assert!(script_class.class_id == cell_class_id);

                // Create a subclass of this class with different instance variables
                match args[0] {
                    TalkValue::Selector(args)   => TalkScriptClassClass::subclass_with_instance_variables(*SCRIPT_CLASS_CLASS, Self::cell_class_reference(cell_class_id), script_class, args.to_signature()),
                    _                           => { args.release_in_context(talk_context); TalkError::NotASelector.into() },
                }
            })
        }
    }

    ///
    /// Generate the function to use for the 'superclass' message
    ///
    fn define_superclass(cell_class_id: TalkClass) -> impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TalkClass, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static> {
        move |_, _, _| {
            TalkContinuation::read_value::<TalkScriptClassClass, _>(Self::cell_class_reference(cell_class_id).into(), move |script_class, _| {
                debug_assert!(script_class.class_id == cell_class_id);

                // Retrieve the superclass for this class
                if let Some(superclass) = &script_class.superclass_id {
                    let superclass = *superclass;

                    TalkContinuation::soon(move |talk_context| {
                        let superclass = superclass.class_object_in_context(talk_context);
                        superclass.clone_in_context(talk_context).into()
                    })
                } else {
                    TalkValue::Nil.into()
                }
            })
        }
    }

    ///
    /// Generate the function to use for the 'newSuperclass' message
    ///
    fn define_new_superclass(cell_class_id: TalkClass) -> impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TalkClass, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static> {
        move |_, _, _| {
            TalkContinuation::read_value::<TalkScriptClassClass, _>(Self::cell_class_reference(cell_class_id).into(), move |script_class, talk_context| {
                debug_assert!(script_class.class_id == cell_class_id);

                if let Some(superclass) = script_class.superclass_id {
                    // Send the 'new' message to the superclass class ID
                    superclass.send_message_in_context(TalkMessage::Unary(*TALK_MSG_NEW), talk_context)
                } else {
                    ().into()
                }
            })
        }
    }

    ///
    /// Generate the function to use for the 'new' message
    ///
    fn define_new(cell_class_id: TalkClass) -> impl 'static + Send + Sync + for<'a> Fn(TalkOwned<TalkClass, &'a TalkContext>, TalkOwned<SmallVec<[TalkValue; 4]>, &'a TalkContext>, &'a TalkContext) -> TalkContinuation<'static> {
        move |_, _, _| {
            TalkContinuation::read_value::<TalkScriptClassClass, _>(Self::cell_class_reference(cell_class_id).into(), move |script_class, _| {
                debug_assert!(script_class.class_id == cell_class_id);

                // Create a new instance of this class (with empty instance variables)
                let instance_size   = script_class.instance_variables.lock().unwrap().len();
                let class_id        = script_class.class_id;

                if script_class.superclass_script_class.is_some() || script_class.superclass_id.is_some() {
                    TalkContinuation::soon(move |context| {
                        // Send the 'newSuperclass' message to ourselves
                        cell_class_id.send_message_in_context(TalkMessage::Unary(*TALK_MSG_NEWSUPERCLASS), context)
                    }).and_then_soon_if_ok(move |superclass, context| {
                        // Allocate space for this instance
                        let cell_block = context.allocate_cell_block(instance_size);

                        // The first value is always a reference to the superclass
                        context.cell_block_mut(cell_block)[0] = superclass;

                        // The result is a reference to the newly created object (cell block classes use their cell block as the data handle)
                        let handle      = TalkDataHandle(cell_block.0 as _);
                        let reference   = TalkReference(class_id, handle);

                        // Call the 'init' message and return the reference
                        reference.clone_in_context(context).send_message_in_context(TalkMessage::Unary(*TALK_MSG_INIT), context)
                            .and_then(|_| {
                                TalkValue::Reference(reference).into()
                            })
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
            })
        }
    }
}

impl TalkScriptClass {
    ///
    /// Adds a class message that calls the specified block
    ///
    fn add_class_message(&mut self, selector: TalkMessageSignature, block: TalkOwned<TalkValue, &'_ TalkContext>) -> TalkContinuation<'static> {
        let cell_class_id   = self.class_id;
        let context         = block.context();
        let message_handler = block.read_data_in_context::<TalkClassMessageHandler>(context);

        if let Some(message_handler) = message_handler {
            // Fetch the superclass; we retain it later on
            let superclass_value    = self.superclass_script_class.clone();

            // Keep the block associated with this class
            let mut resources   = smallvec![];
            let message_id      = usize::from(TalkMessageSignatureId::from(&selector));
            let old_resources   = self.instance_message_resources.remove(message_id);

            // Also retain the superclass if it's present
            if let Some(superclass_value) = &superclass_value {
                resources.push(superclass_value.clone());
            }

            resources.push(block.leak());
            self.class_message_resources.insert(message_id, resources);

            // Add to the dispatch table for the cell class in the current context
            TalkContinuation::soon(move |context| {
                // Retain the superclass
                if let Some(superclass_value) = &superclass_value {
                    superclass_value.retain(context);
                }

                // Release any old resources
                if let Some(old_resources) = old_resources {
                    // Clean up any old message that might be stored here
                    old_resources.into_iter().for_each(|reference| reference.release(context));
                }

                // Define in the dispatch table
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
    fn add_instance_message(&mut self, selector: TalkMessageSignature, block: TalkOwned<TalkValue, &'_ TalkContext>, instance_variables: Arc<Mutex<TalkSymbolTable>>) -> TalkContinuation<'static> {
        let cell_class_id   = self.class_id;
        let context         = block.context();
        let message_handler = block.read_data_in_context::<TalkInstanceMessageHandler>(context);

        if let Some(message_handler) = message_handler {
            // Keep the block associated with this class
            let message_id      = usize::from(TalkMessageSignatureId::from(&selector));
            let old_resources   = self.instance_message_resources.remove(message_id);

            self.instance_message_resources.insert(message_id, smallvec![block.leak()]);

            // Add to the dispatch table for the cell class in the current context
            TalkContinuation::soon(move |context| {
                // Release any old resources
                if let Some(old_resources) = old_resources {
                    // Clean up any old message that might be stored here
                    old_resources.into_iter().for_each(|reference| reference.release(context));
                }

                // Define in the dispatch table
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
    type Allocator = TalkScriptClassAllocator;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        Self::Allocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, class_id: TalkClass, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
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

                let instance_variables = Arc::clone(&script_class.instance_variables);

                // Store the class using the allocator
                let script_class = allocator.lock().unwrap().store(cell_block_class, script_class);

                // Register the class with the context
                let script_class = TalkReference(class_id, script_class);
                talk_context.declare_cell_block_class(script_class.clone_in_context(talk_context), cell_block_class);

                // Declare the standard methods for the class
                Self::declare_class_messages(talk_context, cell_block_class, None, instance_variables);

                // Define an empty 'init' instance method for the new class
                talk_context.get_callbacks_mut(cell_block_class).dispatch_table.define_message(*TALK_MSG_INIT, |_, _, _| { ().into() });

                // Result is the cell block class
                cell_block_class.class_object_in_context(talk_context).into()
            })

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
}

impl TalkClassDefinition for TalkCellBlockClass {
    /// The type of the data stored by an object of this class
    type Data = TalkCellBlock;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkCellBlockAllocator;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        Arc::new(Mutex::new(TalkCellBlockAllocator { tmp_cell_block: TalkCellBlock(0) }))
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
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _reference: TalkReference, _allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}

impl TalkScriptClassAllocator {
    ///
    /// Creates an empty allocator
    ///
    fn empty() -> Arc<Mutex<TalkScriptClassAllocator>> {
        Arc::new(Mutex::new(TalkScriptClassAllocator { 
            classes:            TalkSparseArray::empty(),
            reference_counts:   TalkSparseArray::empty(),
        }))
    }

    ///
    /// Creates the data object for a cell class
    ///
    /// The data handle is the cell class ID, so it's easy to generate a reference to the class data from the cell class ID this way
    ///
    fn store(&mut self, cell_class_id: TalkClass, data: TalkScriptClass) -> TalkDataHandle {
        let class_id = usize::from(cell_class_id);
        self.classes.insert(class_id, data);
        self.reference_counts.insert(class_id, 1);

        TalkDataHandle(class_id)
    }
}

impl TalkClassAllocator for TalkScriptClassAllocator {
    /// The type of data stored for this class
    type Data = TalkScriptClass;

    ///
    /// Retrieves a reference to the data attached to a handle (panics if the handle has been released)
    ///
    fn retrieve<'a>(&'a mut self, handle: TalkDataHandle) -> &'a mut Self::Data {
        let class_id = usize::from(handle);
        self.classes.get_mut(class_id).unwrap()
    }

    ///
    /// Adds to the reference count for a data handle
    ///
    fn retain(allocator: &Arc<Mutex<Self>>, handle: TalkDataHandle, _context: &TalkContext) {
        let mut allocator   = allocator.lock().unwrap();
        let class_id        = usize::from(handle);

        *allocator.reference_counts.get_mut(class_id).unwrap() += 1;
    }

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    fn release(allocator: &Arc<Mutex<Self>>, handle: TalkDataHandle, context: &TalkContext) -> TalkReleaseAction {
        let dropped_class = {
            let mut allocator   = allocator.lock().unwrap();
            let class_id        = usize::from(handle);

            let ref_count       = allocator.reference_counts.get_mut(class_id).unwrap();

            if *ref_count <= 1 {
                allocator.reference_counts.remove(class_id);
                allocator.classes.remove(class_id)
            } else {
                *ref_count -= 1;
                None
            }
        };

        if let Some(dropped_class) = dropped_class {
            dropped_class.release_in_context(context);
            TalkReleaseAction::Dropped
        } else {
            TalkReleaseAction::Retained
        }
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
    fn retain(_allocator: &Arc<Mutex<Self>>, handle: TalkDataHandle, context: &TalkContext) {
        let cell_block = TalkCellBlock(handle.0 as _);
        context.retain_cell_block(cell_block);
    }

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    #[inline]
    fn release(_allocator: &Arc<Mutex<Self>>, handle: TalkDataHandle, context: &TalkContext) -> TalkReleaseAction {
        let cell_block = TalkCellBlock(handle.0 as _);
        context.release_cell_block(cell_block)
    }
}

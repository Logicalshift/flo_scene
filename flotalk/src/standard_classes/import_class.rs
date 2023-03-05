use crate::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use smallvec::*;
use once_cell::sync::{Lazy};

use std::collections::{HashMap};
use std::sync::*;

pub (crate) static IMPORT_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkImportClass));

///
/// The `Import` class is used to request values loaded externally
///
/// This is used to import single items from external modules. For example, `Import item: 'terminalOut' from: 'Terminal'.`
///
/// The importer class manages multiple importers, which maps module names to module objects. Module objects just respond
/// to the `at:` message by returning the corresponding item. Importers can be added to a context by evaluating the
/// continuation returned by the `TalkImportClass::define_high_prority_importer()` or 
/// `TalkImportClass::define_low_priority_importer()` functions.
///
/// ```ignore
/// TalkImportClass::define_low_priority_importer(move |module_name| async move {
///     if &module_name == "TestModule" {
///         Some(TalkContinuation::soon(|_| /* ... Create test module object ... */))
///     } else {
///         None
///     }
/// })
/// ```
///
pub struct TalkImportClass;

///
/// The Import allocator manages the data associated with the TalkImportClass
///
/// TalkImportClass doesn't generate instances as usual (its data type is `()`), but the allocator manages importers within a context.
///
pub struct TalkImportAllocator {
    allocator: TalkStandardAllocator<()>,

    /// Returns an importer for a module. The result of the continuation is either 'nil' to indicate that the exporter did not load a
    /// module, or an object that responds to the `at:` message to return the exported items. The first importer to respond will define
    /// the entire module.
    ///
    /// Once a module has been loaded, it will be cached, and this won't be consulted again
    importers: Vec<Arc<dyn Send + Sync + Fn(String) -> BoxFuture<'static, Option<TalkContinuation<'static>>>>>,

    /// The modules that have been loaded from the importers. 
    modules: HashMap<String, TalkValue>,
}

impl TalkImportClass {
    ///
    /// Defines a new importer function, which will defined at a high priority (if there are other importers that define the same module, this one will be checked first)
    ///
    /// Note that if a module is already loaded, this won't unload it for the importer
    ///
    pub fn define_high_priority_importer<TFuture>(importer: impl 'static + Send + Sync + Fn(String) -> TFuture) -> TalkContinuation<'static>
    where
        TFuture: 'static + Send + Future<Output=Option<TalkContinuation<'static>>>,
    {
        Self::define_importer(true, importer)
    }

    ///
    /// Defines a new importer function, which will defined at a low priority (will only be loaded if the existing importers don't provide this module)
    ///
    pub fn define_low_priority_importer<TFuture>(importer: impl 'static + Send + Sync + Fn(String) -> TFuture) -> TalkContinuation<'static> 
    where
        TFuture: 'static + Send + Future<Output=Option<TalkContinuation<'static>>>,
    {
        Self::define_importer(false, importer)
    }

    ///
    /// Defines a new importer function in the context the returned continuation is evaluated in
    ///
    fn define_importer<TFuture>(check_first: bool, importer: impl 'static + Send + Sync + Fn(String) -> TFuture) -> TalkContinuation<'static> 
    where
        TFuture: 'static + Send + Future<Output=Option<TalkContinuation<'static>>>,
    {
        TalkContinuation::soon(move |context| {
            // Call get_callbacks_mut() to make sure the import class is loaded
            context.get_callbacks_mut(*IMPORT_CLASS);

            // Fetch the allocator
            let import_allocator        = IMPORT_CLASS.allocator::<TalkImportAllocator>(context);
            let import_allocator        = if let Some(import_allocator) = import_allocator { import_allocator } else { return ().into(); };
            let mut import_allocator    = import_allocator.lock().unwrap();

            let importer = move |module_name| {
                importer(module_name).boxed()
            };

            // Define the importer function in the importer
            if check_first {
                import_allocator.importers.insert(0, Arc::new(importer));
            } else {
                import_allocator.importers.push(Arc::new(importer));
            }

            // Result is just nil
            ().into()
        })
    }

    ///
    /// Attempts to load a module from the importer in the continuation context (returning the module object, which responds to the `at:` message or nil
    /// if the module is not available)
    ///
    pub fn load_module(module_name: impl Into<String>) -> TalkContinuation<'static> {
        let module_name = module_name.into();

        TalkContinuation::soon(move |context| {
            // Get the import allocator from the context
            let import_allocator = IMPORT_CLASS.allocator::<TalkImportAllocator>(context);
            let import_allocator = if let Some(import_allocator) = import_allocator { import_allocator } else { return ().into(); };

            // If the module is already loaded, then just use that module
            let import_allocator = import_allocator.lock().unwrap();

            if let Some(module) = import_allocator.modules.get(&module_name) {
                // Return the module previously loaded
                module.clone_in_context(context).into()
            } else {
                // Try to load from each importer in turn
                let all_importers = import_allocator.importers.clone().into_iter();

                Self::try_load_module(module_name, all_importers)
            }
        })
    }

    ///
    /// Attempts to load a module
    ///
    fn try_load_module(module_name: String, importers: impl 'static + Send + Iterator<Item=Arc<dyn Send + Sync + Fn(String) -> BoxFuture<'static, Option<TalkContinuation<'static>>>>>) -> TalkContinuation<'static> {
        let mut importers = importers;

        if let Some(next_importer) = importers.next() {
            // Try to load this next importer
            TalkContinuation::future_soon(async move {
                if let Some(import_module) = next_importer(module_name.clone()).await {
                    // Try to load this module
                    import_module.and_then_soon_if_ok(move |module, context| {
                        // Module should return non-nil to have been successfully loaded
                        if module.is_nil() {
                            // Did not load a module: try the next importer
                            return Self::try_load_module(module_name, importers);
                        }

                        // Loaded a module
                        let import_allocator        = IMPORT_CLASS.allocator::<TalkImportAllocator>(context);
                        let import_allocator        = if let Some(import_allocator) = import_allocator { import_allocator } else { return ().into(); };
                        let mut import_allocator    = import_allocator.lock().unwrap();

                        // Store the module in the cache
                        import_allocator.modules.insert(module_name, module.clone_in_context(context));

                        // Result is the module we just loaded
                        module.into()
                    })
                } else {
                    // Module does not exist: try to load the next module
                    Self::try_load_module(module_name, importers)
                }
            })
        } else {
            // Module could not be loaded
            ().into()
        }
    }
}

impl TalkClassDefinition for TalkImportClass {
    /// The type of the data stored by an object of this class
    type Data = ();

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkStandardAllocator<()>;

    ///
    /// Creates the allocator for this class in a particular context
    ///
    /// This is also an opportunity for a class to perform any other initialization it needs to do within a particular `TalkContext`
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        TalkStandardAllocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        static MSG_ITEM_FROM:   Lazy<TalkMessageSignatureId> = Lazy::new(|| ("item:", "from:").into());

        if message_id == *MSG_ITEM_FROM {
            let mut args    = args;
            let item        = args[0].take();
            let from        = args[1].take();

            if let Ok(from) = from.try_as_string() {
                // Try to load the module
                Self::load_module((*from).clone())
                    .and_then_soon(move |module, context| {
                        if module.is_nil() {
                            // Module could not be found
                            item.release_in_context(context);
                            TalkError::ImportModuleNotFound.into()
                        } else if let Ok(err) = module.try_as_error() {
                            // Some kind of error occurred while loading the module
                            item.release_in_context(context);
                            err.into()
                        } else {
                            // Module loaded OK: send the item message to the module to generate the final result
                            module.send_message(TalkMessage::WithArguments(*TALK_MSG_AT, smallvec![item]))
                        }
                    })
            } else {
                // Expected string
                item.release_in_context(args.context());
                from.release_in_context(args.context());

                TalkError::NotAString.into()
            }
        } else {

            TalkError::MessageNotSupported(message_id).into()
        }
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _reference: TalkReference, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }
}

impl TalkClassAllocator for TalkImportAllocator {
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

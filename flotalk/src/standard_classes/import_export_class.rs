use crate::*;

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
pub struct TalkImportClass;

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

}

///
/// The Import allocator manages the data associated with the TalkImportClass
///
/// TalkImportClass doesn't generate instances as usual (its data type is `()`), but the allocator manages importers within a context.
///
pub struct TalkImportAllocator {
    allocator: TalkStandardAllocator<()>,

    /// Returns an importer for a module. The result of the continuation is either 'nil' to indicate that the exporter did not load a
    /// module, or an object that responds to the `item:` message to return the exported items. The first importer to respond will define
    /// the entire module.
    ///
    /// Once a module has been loaded, it will be cached, and this won't be consulted again
    importers: Vec<Arc<dyn Send + Sync + Fn(&str) -> BoxFuture<'static, Option<TalkContinuation<'static>>>>>,

    /// The modules that have been loaded from the importers. 
    modules: HashMap<String, TalkValue>,
}

impl TalkImportClass {
    ///
    /// Attempts to load a module from the importer in the continuation context (returning the module object, which responds to the `item:` message or nil
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
    fn try_load_module(module_name: String, importers: impl 'static + Send + Iterator<Item=Arc<dyn Send + Sync + Fn(&str) -> BoxFuture<'static, Option<TalkContinuation<'static>>>>>) -> TalkContinuation<'static> {
        let mut importers = importers;

        if let Some(next_importer) = importers.next() {
            // Try to load this next importer
            TalkContinuation::future_soon(async move {
                if let Some(import_module) = next_importer(&module_name).await {
                    // Try to load this module
                    import_module.and_then_soon(move |module, context| {
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

impl TalkClassDefinition for TalkExportClass {
    /// The type of the data stored by an object of this class
    type Data = ();

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkImportAllocator;

    ///
    /// Creates the allocator for this class in a particular context
    ///
    /// This is also an opportunity for a class to perform any other initialization it needs to do within a particular `TalkContext`
    ///
    fn create_allocator(&self, _talk_context: &mut TalkContext) -> Arc<Mutex<Self::Allocator>> {
        Arc::new(Mutex::new(TalkImportAllocator { 
            allocator:  TalkStandardAllocator::new(), 
            importers:  vec![],
            modules:    HashMap::new(),
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

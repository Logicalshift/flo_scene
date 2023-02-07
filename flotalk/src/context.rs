use super::allocator::*;
use super::class::*;
use super::continuation::*;
use super::dispatch_table::*;
use super::message::*;
use super::reference::*;
use super::releasable::*;
use super::sparse_array::*;
use super::standard_classes::*;
use super::symbol::*;
use super::symbol_table::*;
use super::value::*;
use super::value_messages::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use futures::future::{BoxFuture};
use futures::task;

use std::collections::{HashSet, HashMap};
use std::sync::*;
use std::sync::atomic::{AtomicU32, Ordering};

/// All of the cell block classes that have been registered (we want to re-use class IDs between contexts so that the number of class 
/// IDs in existence doesn't keep growing when new contexts are allocated and run scripts)
static CELL_BLOCK_CLASSES: Lazy<Mutex<Vec<TalkClass>>> = Lazy::new(|| Mutex::new(vec![]));

///
/// Class that has been declare
///
pub (super) struct TalkContextCellBlockClass {
    /// The reference to the parent class for this object (retained when this is created, and for as long as at least one instance exists)
    class_object: TalkReference,

    /// The TalkCellBlockClass that is used to represent the instances of this class
    instance_class: TalkClass,
}

///
/// Storage for a 'cell block', a set of values (used for frames and instance variables)
///
struct TalkCellBlockStore {
    /// The values in this cell block
    values: Box<[TalkValue]>,
}

///
/// Manages the background tasks in a TalkContext
///
pub (crate) struct TalkContextBackgroundTasks {
    /// The next ID to assign to a continuation
    next_id: usize,

    /// Continuations that are awaiting to be scheduled
    pub (crate) new_continuations: HashMap<usize, TalkContinuation<'static>>,

    /// Futures that are running on in the background (representing scheduled continuations)
    pub (crate) running_futures: HashMap<usize, Option<BoxFuture<'static, ()>>>,

    /// The futures that have received a wakeup request (or which are newly added and not polled yet)
    pub (crate) awake_futures: HashSet<usize>,

    /// Set to true once the context has been dropped
    pub (crate) context_dropped: bool,

    /// General waker, used to wake up the background tasks when a new task is added or if the main context is dropped
    pub (crate) waker: Option<task::Waker>,
}

///
/// A talk context is a self-contained representation of the state of a flotalk interpreter
///
/// Contexts are only accessed on one thread at a time. They're wrapped by a `TalkRuntime`, which deals with
/// scheduling continuations on a context
///
pub struct TalkContext {
    /// Callbacks for this context, indexed by class ID
    context_callbacks: Vec<Option<TalkClassContextCallbacks>>,

    /// Dispatch tables by value
    pub (super) value_dispatch_tables: TalkValueDispatchTables,

    /// Background tasks queued on this context
    pub (super) background_tasks: Arc<Mutex<TalkContextBackgroundTasks>>,

    /// Messages that should be dispatched as an inverted message via the `TalkInvertedClass`
    inverted_messages: TalkSparseArray<()>,

    /// Callbacks made when a reference is dropped in this context
    when_dropped: Vec<Box<dyn Send + Fn(TalkReference, &TalkContext) -> ()>>,

    /// Vec indexed by class of the superclass of any given type
    superclass: Vec<Option<TalkClass>>,

    /// Storage cells that make up the heap for the interpreter
    cells: Vec<TalkCellBlockStore>,

    /// The reference count for each cell block (this allows us to share cell blocks around more easily)
    cell_reference_count: Vec<AtomicU32>,

    /// These are the classes that have been declared in this context that have a separate class object
    cell_block_classes: Vec<TalkContextCellBlockClass>,

    /// The index of the first unused cell block class from the static CELL_BLOCK_CLASSES list (used to reallocate class IDs)
    next_cell_block_class_idx: usize,

    /// These classes are all of type TalkCellBlockClass, and are used for storing instance variables and custom-defined methods for script classes
    available_cell_block_classes: Vec<TalkClass>,

    /// Values in the 'cells' array that have been freed
    free_cells: Mutex<Vec<usize>>,

    /// The cell block containing the values for the root symbol table
    root_cell_block: TalkCellBlock,

    /// The 'root' symbol table, which can be used for binding symbols when they have no symbol table of their own
    root_symbol_table: Arc<Mutex<TalkSymbolTable>>,
}

impl TalkContext {
    ///
    /// Creates a new, empty context
    ///
    pub fn empty() -> TalkContext {
        TalkContext {
            context_callbacks:              vec![],
            value_dispatch_tables:          TalkValueDispatchTables::default(),
            background_tasks:               Arc::new(Mutex::new(TalkContextBackgroundTasks::new())),
            inverted_messages:              TalkSparseArray::empty(),
            when_dropped:                   vec![],
            superclass:                     vec![],
            cells:                          vec![TalkCellBlockStore { values: Box::new([]) }],
            cell_reference_count:           vec![AtomicU32::new(1)],
            cell_block_classes:             vec![],
            available_cell_block_classes:   vec![],
            next_cell_block_class_idx:      0,
            free_cells:                     Mutex::new(vec![]),
            root_cell_block:                TalkCellBlock(0),
            root_symbol_table:              Arc::new(Mutex::new(TalkSymbolTable::empty())),
        }
    }

    ///
    /// Creates the callbacks for a particular class
    ///
    fn create_callbacks<'a>(&'a mut self, class: TalkClass) -> &'a mut TalkClassContextCallbacks {
        let TalkClass(class_id) = class;

        while self.context_callbacks.len() <= class_id {
            self.context_callbacks.push(None);
        }

        let class_callbacks     = class.callbacks();
        let context_callbacks   = class_callbacks.create_in_context(self);

        self.context_callbacks[class_id] = Some(context_callbacks);
        self.context_callbacks[class_id].as_mut().unwrap()
    }

    ///
    /// Retrieves the callbacks for a class
    ///
    #[inline]
    pub (super) fn get_callbacks_mut<'a>(&'a mut self, class: TalkClass) -> &'a mut TalkClassContextCallbacks {
        let TalkClass(class_id) = class;

        if self.context_callbacks.len() > class_id {
            if self.context_callbacks[class_id].is_some() {
                return self.context_callbacks[class_id].as_mut().unwrap()
            }
        }

        self.create_callbacks(class)
    }

    ///
    /// Retrieves the callbacks for a class
    ///
    #[inline]
    pub (super) fn get_callbacks<'a>(&'a self, class: TalkClass) -> Option<&'a TalkClassContextCallbacks> {
        let TalkClass(class_id) = class;

        match self.context_callbacks.get(class_id) {
            Some(ctxt)  => ctxt.as_ref(),
            None        => None,
        }
    }

    ///
    /// For a reference, retrieves the dispatch table for its class implementation
    ///
    /// If the reference is to a class object, then this is the dispatch table for that class, rather than the class object itself.
    /// If the reference is to an instance of an object, then this is the dispatch table for that class of that object.
    ///
    #[inline]
    pub fn class_dispatch_table<'a>(&'a mut self, reference: &TalkReference) -> &'a mut TalkMessageDispatchTable<TalkClass> {
        if reference.is_class_object() {
            let class       = TalkClass(reference.1.into());
            let callbacks   = self.get_callbacks_mut(class);

            &mut callbacks.class_dispatch_table
        } else {
            let class       = reference.0;
            let callbacks   = self.get_callbacks_mut(class);

            &mut callbacks.class_dispatch_table
        }
    }

    ///
    /// For a reference, retrieves the dispatch table for its instance implementation
    ///
    /// If the reference is to a class object, then this is the dispatch table for that class, rather than the class object itself.
    /// If the reference is to an instance of an object, then this is the dispatch table for that class of that object.
    ///
    #[inline]
    pub fn instance_dispatch_table<'a>(&'a mut self, reference: &TalkReference) -> &'a mut TalkMessageDispatchTable<TalkReference> {
        if reference.is_class_object() {
            let class       = TalkClass(reference.1.into());
            let callbacks   = self.get_callbacks_mut(class);

            &mut callbacks.dispatch_table
        } else {
            let class       = reference.0;
            let callbacks   = self.get_callbacks_mut(class);

            &mut callbacks.dispatch_table
        }
    }

    ///
    /// Releases multiple references using this context
    ///
    #[inline]
    pub fn release_references(&self, references: impl IntoIterator<Item=TalkReference>) {
        for reference in references {
            if let Some(callbacks) = self.get_callbacks(reference.0) {
                callbacks.release(reference.1, self);
            }
        }
    }

    ///
    /// Releases multiple references using this context
    ///
    #[inline]
    pub fn release_values<'a>(&self, values: impl IntoIterator<Item=&'a TalkValue>) {
        for value in values {
            match value {
                TalkValue::Reference(reference) => {
                    if let Some(callbacks) = self.get_callbacks(reference.0) {
                        callbacks.release(reference.1, self);
                    }
                },

                TalkValue::Array(array) => {
                    self.release_values(array);
                }

                _ => {}
            }
        }
    }

    ///
    /// Allocates a block of cells, returning the size
    ///
    /// The block is returned with a reference count of 1
    ///
    #[inline]
    pub fn allocate_cell_block(&mut self, count: usize) -> TalkCellBlock {
        // Crete a new block of nil cells
        let new_block = (0..count).into_iter().map(|_| TalkValue::Nil).collect::<Vec<_>>().into_boxed_slice();
        let new_block = TalkCellBlockStore { 
            values: new_block,
        };

        // Store at the end of the list of cells or add a new item to the list
        if let Some(idx) = self.free_cells.lock().unwrap().pop() {
            self.cells[idx]                 = new_block;
            self.cell_reference_count[idx]  = AtomicU32::new(1);
            TalkCellBlock(idx as _)
        } else {
            let idx = self.cells.len();
            self.cells.push(new_block);
            self.cell_reference_count.push(AtomicU32::new(1));
            TalkCellBlock(idx as _)
        }
    }

    ///
    /// Sets the size of an allocated cell block
    ///
    pub fn resize_cell_block(&mut self, TalkCellBlock(idx): TalkCellBlock, new_size: usize) {
        use std::mem;

        // Create an empty block
        let new_block: Box<[TalkValue]> = Box::new([]);
        let mut new_block               = TalkCellBlockStore {
            values: new_block,
        };

        // Convert the existing block back to a vec
        mem::swap(&mut self.cells[idx as usize], &mut new_block);
        let mut new_block = new_block.values.to_vec();

        // Reserve space for the new cells
        if new_size > new_block.len() {
            new_block.reserve_exact(new_size - new_block.len());

            while new_block.len() < new_size {
                new_block.push(TalkValue::Nil);
            }
        } else {
             while new_block.len() > new_size {
                new_block.pop();
             }

             new_block.shrink_to_fit();
        }

        // Convert back to a slice
        let new_block       = new_block.into_boxed_slice();
        let mut new_block   = TalkCellBlockStore {
            values: new_block,
        };

        // Put back in to the cells
        mem::swap(&mut self.cells[idx as usize], &mut new_block);
    }

    ///
    /// Retains a cell block so that 'release' needs to be called on it one more time
    ///
    pub fn retain_cell_block(&self, TalkCellBlock(idx): TalkCellBlock) {
        let old_count = self.cell_reference_count[idx as usize].fetch_add(1, Ordering::Relaxed);
        debug_assert!(old_count > 0);
    }

    ///
    /// Releases the contents of a set of cells
    ///
    fn release_cell_contents(&self, cells: &Box<[TalkValue]>) {
        cells.into_iter()
            .for_each(|value| value.release(self));
    }

    ///
    /// Releases a block of cells, freeing it if its reference count reaches 0
    ///
    /// Returns true if the cell block was actually released, otherwise false
    ///
    #[inline]
    pub fn release_cell_block(&self, TalkCellBlock(idx): TalkCellBlock) -> TalkReleaseAction {
        let ref_count = &self.cell_reference_count[idx as usize];
        debug_assert!(ref_count.load(Ordering::Relaxed) > 0);

        let old_count = ref_count.fetch_sub(1, Ordering::Relaxed);
        if old_count == 1 {
            // The old cells are left behind (as we can't mutate them here) but we reduce their reference count too. References may start to point at invalid values.
            // Once the cells are reallocated using allocate_cell_block, their contents are finally fully freed
            let freed_cells = &self.cells[idx as usize];
            self.release_cell_contents(&freed_cells.values);

            self.free_cells.lock().unwrap().push(idx as _);

            // This makes the cell block 'dropped' so it can't be accessed any more
            TalkReleaseAction::Dropped
        } else {
            // Other owners of the cell block may still access it
            TalkReleaseAction::Retained
        }
    }

    ///
    /// Retrieves a cell block for reading
    ///
    #[inline]
    pub fn cell_block(&self, TalkCellBlock(idx): &TalkCellBlock) -> &[TalkValue] {
        &self.cells[*idx as usize].values
    }

    ///
    /// Retrieves a cell block for writing
    ///
    #[inline]
    pub fn cell_block_mut(&mut self, TalkCellBlock(idx): &TalkCellBlock) -> &mut [TalkValue] {
        &mut self.cells[*idx as usize].values
    }

    ///
    /// Returns a reference to the value in a cell
    ///
    #[inline]
    pub fn get_cell(&self, TalkCell(TalkCellBlock(block_idx), cell_idx): TalkCell) -> &TalkValue {
        &self.cells[block_idx as usize].values[cell_idx as usize]
    }

    ///
    /// Returns a mutable reference to the value in a cell
    ///
    #[inline]
    pub fn get_cell_mut(&mut self, TalkCell(TalkCellBlock(block_idx), cell_idx): TalkCell) -> &mut TalkValue {
        &mut self.cells[block_idx as usize].values[cell_idx as usize]
    }

    ///
    /// Creates an empty (clear dispatch table) instance of a cell block class in this context
    ///
    /// The TalkClass may have been used before by a cell block class that no longer has any instances or references to it. It may also
    /// be in use in other contexts elsewhere in the application for classes with a different identity. Cell block classes are useful
    /// for scripting as the data handle can be directly used as the ID for a TalkCellBlock.
    ///
    pub (super) fn empty_cell_block_class(&mut self) -> TalkClass {
        if let Some(existing_class) = self.available_cell_block_classes.pop() {
            // Flush the dispatch tables for this class
            let callbacks                   = self.get_callbacks_mut(existing_class);
            callbacks.dispatch_table        = TalkMessageDispatchTable::empty();
            callbacks.class_dispatch_table  = TalkMessageDispatchTable::empty();

            existing_class
        } else {
            // Try to use a cell block class that's already in use
            let mut existing_classes = CELL_BLOCK_CLASSES.lock().unwrap();

            if self.next_cell_block_class_idx < existing_classes.len() {
                // There are still unused classes in the 'existing' list
                let class_idx = self.next_cell_block_class_idx;
                self.next_cell_block_class_idx += 1;

                existing_classes[class_idx]
            } else {
                // Create a new cell block class
                let new_class = TalkClass::create(TalkCellBlockClass);

                // Add to the existing class list
                existing_classes.push(new_class);
                self.next_cell_block_class_idx = existing_classes.len();

                new_class
            }
        }
    }

    ///
    /// Associates a class object with its instance classes (used for freeing up the cell block classes when there are no 
    /// more instances left, and for retaining the class object while instances still exist)
    ///
    /// The context will take ownership of the reference, and will free it once there are no more instances of the cell block
    /// class left.
    ///
    pub (super) fn declare_cell_block_class(&mut self, class_object: TalkReference, cell_block_instance_class: TalkClass) {
        self.cell_block_classes.push(TalkContextCellBlockClass {
            class_object:   class_object,
            instance_class: cell_block_instance_class,  
        })
    }

    ///
    /// Returns the shared root symbol table for this context
    ///
    #[inline]
    pub fn root_symbol_table(&self) -> Arc<Mutex<TalkSymbolTable>> {
        self.root_symbol_table.clone()
    }

    ///
    /// Retrieves the cell block containing the values for the root symbol table
    ///
    #[inline]
    pub fn root_symbol_table_cell_block<'a>(&'a self) -> TalkOwned<TalkCellBlock, &'a TalkContext> {
        let cell_block = self.root_cell_block.clone();
        self.retain_cell_block(cell_block);
        TalkOwned::new(cell_block, self)
    }

    ///
    /// Replaces the existing root symbol table with a new empty one
    ///
    pub fn create_empty_root_symbol_table(&mut self) {
        // Create a new symbol table
        self.root_symbol_table = Arc::new(Mutex::new(TalkSymbolTable::empty()));

        // Free the old cell block and replace it with a new empty one
        self.release_cell_block(self.root_cell_block);
        self.root_cell_block = self.allocate_cell_block(128);
    }

    ///
    /// Sets the value of a symbol in the root symbol table (defining it if necessary)
    ///
    pub fn set_root_symbol_value<'a>(&mut self, symbol: impl Into<TalkSymbol>, new_value: TalkValue) {
        // Define a new cell or retrieve the existing cell for the symbol
        let symbol_index = {
            let symbol                  = symbol.into();
            let mut root_symbol_table   = self.root_symbol_table.lock().unwrap();
            let symbol_index            = if let Some(existing_symbol) = root_symbol_table.symbol(symbol) { existing_symbol } else { root_symbol_table.define_symbol(symbol) };

            symbol_index
        };
        let symbol_index = symbol_index.cell as usize;

        // Make sure that there are enough cells defined
        let root_cell_block = self.cell_block(&self.root_cell_block);

        if root_cell_block.len() <= symbol_index {
            // Decide how big to make the new root cell block
            let mut new_size = root_cell_block.len();
            while new_size <= symbol_index {
                new_size *= 2;
                if new_size == 0 { new_size = 128 }
            }

            // Resize the cell block
            self.resize_cell_block(self.root_cell_block, new_size);
        }

        // Re-fetch the root cell block
        let root_cell_block = self.root_cell_block.clone();
        let root_cell_block = self.cell_block_mut(&root_cell_block);

        // Release any existing value and store the new value
        let old_value = root_cell_block[symbol_index].take();
        root_cell_block[symbol_index] = new_value;
        old_value.release_in_context(self);
    }

    ///
    /// Runs a continuation in the background (alongside any other continuations that might be running). Any result it produces will be discarded
    ///
    pub fn run_in_background(&self, continuation: TalkContinuation<'static>) {
        TalkContextBackgroundTasks::add_background_continuation(&self.background_tasks, continuation);
    }

    ///
    /// Adds an action that will be performed whenever a reference is dropped in this context
    ///
    /// Note: the reference passed in here has already been dropped at the point the callback is made, so it
    /// is not safe to attempt to retrieve any data for it.
    ///
    pub fn on_dropped_reference(&mut self, action: impl 'static + Send + Fn(TalkReference, &TalkContext) -> ()) {
        self.when_dropped.push(Box::new(action))
    }

    ///
    /// Indicates that a reference has been dropped (is no longer valid)
    ///
    pub (super) fn notify_dropped(&self, reference: TalkReference) {
        for on_drop in self.when_dropped.iter() {
            let dropped_reference = TalkReference(reference.0, reference.1);
            on_drop(dropped_reference, self);
        }
    }

    ///
    /// Sets the superclass of a particular talk class ID
    ///
    pub fn set_superclass(&mut self, class: TalkClass, superclass: TalkClass) {
        let class_id = usize::from(class);

        // Extend the superclass list until we have space to store our class's superclass
        while self.superclass.len() <= class_id {
            self.superclass.push(None);
        }

        // Store the superclass for this class
        self.superclass[class_id] = Some(superclass);
    }

    ///
    /// Reads the superclass of a class ID
    ///
    #[inline]
    pub fn superclass(&self, class: TalkClass) -> Option<TalkClass> {
        let class_id = usize::from(class);

        if class_id < self.superclass.len() {
            self.superclass[class_id]
        } else {
            None
        }
    }

    ///
    /// Adds a message type that should be dispatched 'inverted' via the TalkInvertedClass object
    ///
    #[inline]
    pub fn add_inverted_message(&mut self, message: impl Into<TalkMessageSignatureId>) {
        // Make sure that the inverted class is in memory, we need it to do the actual sending later on
        self.get_callbacks_mut(*INVERTED_CLASS);

        // Add an entry for this message so we'll send messages to it inverted
        let message = message.into();
        self.inverted_messages.insert(message.into(), ());
    }

    ///
    /// Attempts to send a message using the `Inverted` class, if it can be sent that way (returns the source and message back in the error if the message is not supported as inverted)
    ///
    #[inline]
    pub fn try_send_inverted_message<'a>(&self, source: TalkOwned<TalkValue, &'a Self>, message: TalkOwned<TalkMessage, &'a Self>) -> Result<TalkContinuation<'static>, (TalkOwned<TalkValue, &'a Self>, TalkOwned<TalkMessage, &'a Self>)> {
        if self.inverted_messages.get(message.signature_id().into()).is_some() {
            // The source must currently be a TalkReference as we don't have class objects for values yet
            let context = source.context();
            let source  = source.leak();
            let source  = match source {
                TalkValue::Reference(reference) => Ok(TalkOwned::new(reference, context)),
                _                               => Err(TalkOwned::new(source, context)),
            };

            match source {
                Ok(source) => {
                    // This message is marked as supported in this context
                    Ok(TalkInvertedClass::send_inverted_message(self, source, message))
                }

                Err(source) => {
                    // Can't send inverted messages to pure values at the moment
                    Err((source, message))
                }
            }
        } else {
            // Can't send this as an inverted message, so just return the source and the message back again
            Err((source, message))
        }
    }

    ///
    /// Attempts to send a message using the `Inverted` class, if it can be sent that way (returns the source and message back in the error if the message is not supported as inverted)
    ///
    #[inline]
    pub (super) fn try_send_inverted_message_reference<'a>(&self, source: TalkOwned<TalkReference, &'a Self>, message_id: TalkMessageSignatureId, message_args: TalkOwned<SmallVec<[TalkValue; 4]>, &'a Self>) -> Result<TalkContinuation<'static>, (TalkOwned<TalkReference, &'a Self>, TalkMessageSignatureId, TalkOwned<SmallVec<[TalkValue; 4]>, &'a Self>)> {
        if self.inverted_messages.get(message_id.into()).is_some() {
            // This message is marked as supported in this context
            let context         = message_args.context();
            let message_args    = message_args.leak();
            let message         = if message_args.len() == 0 {
                TalkMessage::Unary(message_id)
            } else {
                TalkMessage::WithArguments(message_id, message_args)
            };

            let message = TalkOwned::new(message, context);
            Ok(TalkInvertedClass::send_inverted_message(self, source, message))
        } else {
            // Can't send this as an inverted message, so just return the source and the message back again
            Err((source, message_id, message_args))
        }
    }
}

impl Drop for TalkContext {
    fn drop(&mut self) {
        TalkContextBackgroundTasks::drop_context(&self.background_tasks);
    }
}

impl TalkContextBackgroundTasks {
    ///
    /// Creates an empty set of background tasks
    ///
    fn new() -> TalkContextBackgroundTasks {
        TalkContextBackgroundTasks {
            next_id:            0,
            new_continuations:  HashMap::new(),
            running_futures:    HashMap::new(),
            awake_futures:      HashSet::new(),
            context_dropped:    false,
            waker:              None,
        }
    }

    ///
    /// Sets the context as dropped and wakes up anything running the background tasks
    ///
    /// Background task runners should keep only a weak reference to the context to make sure that this event can happen
    ///
    fn drop_context(arc_self: &Arc<Mutex<TalkContextBackgroundTasks>>) {
        let waker = {
            let mut background_tasks = arc_self.lock().unwrap();

            background_tasks.context_dropped = true;
            background_tasks.waker.take()
        };

        // Wake up is done outside the lock (to make it impossible for a re-entry to deadlock anything)
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    ///
    /// Adds a background continuation to these background tasks
    ///
    fn add_background_continuation(arc_self: &Arc<Mutex<TalkContextBackgroundTasks>>, continuation: TalkContinuation<'static>) {
        let waker = {
            let mut background_tasks = arc_self.lock().unwrap();

            // Assign an ID to this continuation (this is used so we only poll continuations that are 'awake' so large numbers of background continuations can be efficiently handled)
            let continuation_id         = background_tasks.next_id;
            background_tasks.next_id    += 1;

            // Add to the list of background continuations
            background_tasks.new_continuations.insert(continuation_id, continuation);

            // Mark as awake
            background_tasks.awake_futures.insert(continuation_id);

            // Wake up anything that's running the background continuations so that it runs this continuation for the first time
            background_tasks.waker.take()
        };

        // Wake up is done outside the lock (to make it impossible for a re-entry to deadlock anything)
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn reuse_cell_block_classes_in_different_contexts() {
        let mut context_1 = TalkContext::empty();
        let mut context_2 = TalkContext::empty();

        let class_1 = context_1.empty_cell_block_class();
        let class_2 = context_2.empty_cell_block_class();
        let class_3 = context_2.empty_cell_block_class();
        let class_4 = context_1.empty_cell_block_class();

        assert!(class_1 == class_2);
        assert!(class_3 == class_4);
    }
}

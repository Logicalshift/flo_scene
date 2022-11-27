use super::class::*;
use super::reference::*;
use super::value::*;
use super::value_messages::*;

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

    /// Storage cells that make up the heap for the interpreter
    pub (super) cells: Vec<Option<Box<[TalkValue]>>>,

    /// Values in the 'cells' array that have been freed
    pub (super) free_cells: Vec<usize>,
}

impl TalkContext {
    ///
    /// Creates a new, empty context
    ///
    pub fn empty() -> TalkContext {
        TalkContext {
            context_callbacks:      vec![],
            value_dispatch_tables:  TalkValueDispatchTables::default(),
            cells:                  vec![],
            free_cells:             vec![],
        }
    }

    ///
    /// Creates the allocator for a particular class
    ///
    fn create_callbacks<'a>(&'a mut self, class: TalkClass) -> &'a mut TalkClassContextCallbacks {
        let TalkClass(class_id) = class;

        while self.context_callbacks.len() <= class_id {
            self.context_callbacks.push(None);
        }

        let class_callbacks     = class.callbacks();
        let context_callbacks   = class_callbacks.create_in_context();

        self.context_callbacks[class_id] = Some(context_callbacks);
        self.context_callbacks[class_id].as_mut().unwrap()
    }

    ///
    /// Retrieves the allocator for a class
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
    /// Retrieves the allocator for a class
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
    /// Releases multiple references using this context
    ///
    #[inline]
    pub fn release_references(&self, references: impl IntoIterator<Item=TalkReference>) {
        for reference in references {
            if let Some(callbacks) = self.get_callbacks(reference.0) {
                callbacks.remove_reference(reference.1);
            }
        }
    }

    ///
    /// Releases multiple references using this context
    ///
    #[inline]
    pub fn release_values(&self, values: impl IntoIterator<Item=TalkValue>) {
        for value in values {
            match value {
                TalkValue::Reference(reference) => {
                    if let Some(callbacks) = self.get_callbacks(reference.0) {
                        callbacks.remove_reference(reference.1);
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
    #[inline]
    pub fn allocate_cell_block(&mut self, count: usize) -> usize {
        // Crete a new block of nil cells
        let new_block = (0..count).into_iter().map(|_| TalkValue::Nil).collect::<Vec<_>>().into_boxed_slice();

        // Store at the end of the list of cells or add a new item to the list
        if let Some(idx) = self.free_cells.pop() {
            self.cells[idx] = Some(new_block);
            idx
        } else {
            let idx = self.cells.len();
            self.cells.push(Some(new_block));
            idx
        }
    }

    ///
    /// Frees a block of cells, adding it to the free list
    ///
    #[inline]
    pub fn free_cell_block(&mut self, idx: usize) {
        debug_assert!(self.cells[idx].is_some());

        self.cells[idx] = None;
        self.free_cells.push(idx);
    }

    ///
    /// Retrieves a cell block for reading
    ///
    #[inline]
    pub fn cell_block(&self, idx: usize) -> &[TalkValue] {
        self.cells[idx].as_ref().expect("Can't read a freed cell block")
    }

    ///
    /// Retrieves a cell block for writing
    ///
    #[inline]
    pub fn cell_block_mut(&mut self, idx: usize) -> &mut [TalkValue] {
        self.cells[idx].as_mut().expect("Can't change a freed cell block")
    }
}

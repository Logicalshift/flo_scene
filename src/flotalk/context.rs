use super::class::*;
use super::reference::*;
use super::value::*;
use super::value_messages::*;

use std::sync::*;
use std::sync::atomic::{AtomicU32, Ordering};

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
    cells: Vec<Box<[TalkValue]>>,

    /// The reference count for each cell block (this allows us to share cell blocks around more easily)
    cell_reference_count: Vec<AtomicU32>,

    /// Values in the 'cells' array that have been freed
    free_cells: Mutex<Vec<usize>>,
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
            cell_reference_count:   vec![],
            free_cells:             Mutex::new(vec![]),
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
    /// The block is returned with a reference count of 1
    ///
    #[inline]
    pub fn allocate_cell_block(&mut self, count: usize) -> TalkCellBlock {
        // Crete a new block of nil cells
        let new_block = (0..count).into_iter().map(|_| TalkValue::Nil).collect::<Vec<_>>().into_boxed_slice();

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
    #[inline]
    pub fn resize_cell_block(&mut self, TalkCellBlock(idx): TalkCellBlock, new_size: usize) {
        //use std::mem;

        //let mut new_block = Box::new([]);

        todo!()
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
            .for_each(|value| value.remove_reference(self));
    }

    ///
    /// Releases a block of cells, freeing it if its reference count reaches 0
    ///
    #[inline]
    pub fn release_cell_block(&self, TalkCellBlock(idx): TalkCellBlock) {
        let ref_count = &self.cell_reference_count[idx as usize];
        debug_assert!(ref_count.load(Ordering::Relaxed) > 0);

        let old_count = ref_count.fetch_sub(1, Ordering::Relaxed);
        if old_count == 1 {
            // The old cells are left behind (as we can't mutate them here) but we reduce their reference count too. References may start to point at invalid values.
            // Once the cells are reallocated using allocate_cell_block, their contents are finally fully freed
            let freed_cells = &self.cells[idx as usize];
            self.release_cell_contents(freed_cells);

            self.free_cells.lock().unwrap().push(idx as _);
        }
    }

    ///
    /// Retrieves a cell block for reading
    ///
    #[inline]
    pub fn cell_block(&self, TalkCellBlock(idx): TalkCellBlock) -> &[TalkValue] {
        &self.cells[idx as usize]
    }

    ///
    /// Retrieves a cell block for writing
    ///
    #[inline]
    pub fn cell_block_mut(&mut self, TalkCellBlock(idx): TalkCellBlock) -> &mut [TalkValue] {
        &mut self.cells[idx as usize]
    }

    ///
    /// Returns a reference to the value in a cell
    ///
    #[inline]
    pub fn get_cell(&self, TalkCell(TalkCellBlock(block_idx), cell_idx): TalkCell) -> &TalkValue {
        &self.cells[block_idx as usize][cell_idx as usize]
    }

    ///
    /// Returns a mutable reference to the value in a cell
    ///
    #[inline]
    pub fn get_cell_mut(&mut self, TalkCell(TalkCellBlock(block_idx), cell_idx): TalkCell) -> &mut TalkValue {
        &mut self.cells[block_idx as usize][cell_idx as usize]
    }
}

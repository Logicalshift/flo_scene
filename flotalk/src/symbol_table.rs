use super::reference::*;
use super::sparse_array::*;
use super::symbol::*;

use std::sync::*;

///
/// A symbol table will map TalkSymbols to frame cell references in a specific frame
///
#[derive(Clone)]
pub struct TalkSymbolTable {
    /// The next cell ID to allocate
    next_cell: u32,

    /// The symbols in this frame
    symbols: TalkSparseArray<u32>,

    /// The symbol table for the parent frame (frame 1 relative to this one)
    parent: Option<Arc<Mutex<TalkSymbolTable>>>,
}

impl TalkSymbolTable {
    ///
    /// Creates an empty, top-level symbol table
    ///
    pub fn empty() -> TalkSymbolTable {
        TalkSymbolTable {
            next_cell:  0,
            symbols:    TalkSparseArray::empty(),
            parent:     None,
        }
    }

    ///
    /// Creates a symbol table with a parent frame (symbols from the parent frame will have a frame of 1, from the grandparent it will be 2, etc)
    ///
    pub fn with_parent_frame(parent_frame: Arc<Mutex<TalkSymbolTable>>) -> TalkSymbolTable {
        TalkSymbolTable {
            next_cell:  0,
            symbols:    TalkSparseArray::empty(),
            parent:     Some(parent_frame),
        }
    }

    ///
    /// The number of cells that need to be allocated to represent this symbol table
    ///
    pub fn len(&self) -> usize {
        self.next_cell as usize
    }

    ///
    /// Replaces the parent frame in this symbol table
    ///
    pub fn set_parent_frame(&mut self, parent_frame: Arc<Mutex<TalkSymbolTable>>) {
        self.parent = Some(parent_frame)
    }

    ///
    /// Defines a symbol within this table, assigning it a new cell (including if the symbol is already bound to something)
    ///
    pub fn define_symbol(&mut self, symbol: impl Into<TalkSymbol>) -> TalkFrameCell {
        let TalkSymbol(sym_id) = symbol.into();

        let this_cell   = self.next_cell;
        self.next_cell  += 1;

        self.symbols.insert(sym_id, this_cell);
        TalkFrameCell { frame: 0, cell: this_cell }
    }

    ///
    /// Undefines a symbol within this table (its location will not be re-used)
    ///
    pub fn undefine_symbol(&mut self, symbol: impl Into<TalkSymbol>) {
        let TalkSymbol(sym_id) = symbol.into();

        self.symbols.remove(sym_id);
    }

    ///
    /// Makes a symbol an alias for an existing cell
    ///
    pub fn alias_symbol(&mut self, symbol: impl Into<TalkSymbol>, existing_cell: u32) {
        let TalkSymbol(sym_id) = symbol.into();

        debug_assert!(existing_cell < self.next_cell);

        self.symbols.insert(sym_id, existing_cell);
    }

    ///
    /// Looks up a symbol within this table
    ///
    pub fn symbol(&self, symbol: impl Into<TalkSymbol>) -> Option<TalkFrameCell> {
        let TalkSymbol(sym_id) = symbol.into();

        if let Some(cell) = self.symbols.get(sym_id) {
            Some(TalkFrameCell {
                frame:  0,
                cell:   *cell,
            })
        } else if let Some(parent) = &self.parent {
            if let Some(parent_cell) = parent.lock().unwrap().symbol(TalkSymbol(sym_id)) {
                // TODO: doing this iteratively rather than recursively would be faster
                Some(TalkFrameCell {
                    frame:  parent_cell.frame + 1,
                    cell:   parent_cell.cell,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}
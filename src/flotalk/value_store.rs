use super::symbol::*;
use super::value::*;

use std::collections::{HashMap};

///
/// Associates symbols with values
///
pub struct TalkValueStore {
    /// The values in this store
    values: Vec<TalkValue>,

    /// Maps symbols to their storage cells
    locations: HashMap<TalkSymbol, usize>,
}

impl Default for TalkValueStore {
    fn default() -> TalkValueStore {
        TalkValueStore {
            values:     vec![],
            locations:  HashMap::new(),
        }
    }
}

impl TalkValueStore {
    ///
    /// Retrieves the value at the specified location
    ///
    #[inline]
    pub fn at_location(&mut self, location: usize) -> &mut TalkValue {
        &mut self.values[location]
    }

    ///
    /// Defines a new symbol location
    ///
    /// Overwrites the existing symbol location if it exists, or creates a new location with a nil value if it doesn't
    ///
    pub fn define_symbol(&mut self, symbol: impl Into<TalkSymbol>) -> usize {
        let offset = self.values.len();
        self.values.push(TalkValue::Nil);

        self.locations.insert(symbol.into(), offset);

        offset
    }

    ///
    /// Updates the location of a symbol to a new location
    ///
    pub fn set_symbol_location(&mut self, symbol: impl Into<TalkSymbol>, new_location: usize) {
        self.locations.insert(symbol.into(), new_location);
    }

    ///
    /// Undefines a symbol and returns its old location
    ///
    pub fn undefine_symbol(&mut self, symbol: impl Into<TalkSymbol>) -> Option<usize> {
        self.locations.remove(&symbol.into())
    }

    ///
    /// Finds the location for a symbol, if it has one
    ///
    pub fn location_for_symbol(&self, symbol: impl Into<TalkSymbol>) -> Option<usize> {
        self.locations.get(&symbol.into()).copied()
    }

    ///
    /// Finds the value for a symbol, if it has one
    ///
    pub fn value_for_symbol(&mut self, symbol: impl Into<TalkSymbol>) -> Option<&mut TalkValue> {
        self.location_for_symbol(symbol).map(|location| self.at_location(location))
    }
}

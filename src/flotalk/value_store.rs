use super::context::*;
use super::symbol::*;
use super::value::*;

use std::collections::{HashMap};

///
/// Associates symbols with values
///
pub struct TalkValueStore<TValue> {
    /// The values in this store
    values: Vec<TValue>,

    /// Maps symbols to their storage cells
    locations: HashMap<TalkSymbol, usize>,
}

impl<TValue> Default for TalkValueStore<TValue> {
    fn default() -> TalkValueStore<TValue> {
        TalkValueStore {
            values:     vec![],
            locations:  HashMap::new(),
        }
    }
}

impl<TValue> TalkValueStore<TValue> 
where
    TValue: Default,
{
    ///
    /// Defines a new symbol location
    ///
    /// Overwrites the existing symbol location if it exists, or creates a new location with a nil value if it doesn't
    ///
    pub fn define_symbol(&mut self, symbol: impl Into<TalkSymbol>) -> usize {
        let offset = self.values.len();
        self.values.push(TValue::default());

        self.locations.insert(symbol.into(), offset);

        offset
    }
}

impl<TValue> TalkValueStore<TValue> {
    ///
    /// Retrieves the value at the specified location
    ///
    #[inline]
    pub fn at_location(&mut self, location: usize) -> &mut TValue {
        &mut self.values[location]
    }

    ///
    /// Sets the value of a symbol (defining it in this store if it's not already defined)
    ///
    pub fn set_symbol_value(&mut self, symbol: impl Into<TalkSymbol>, new_value: TValue) {
        let symbol = symbol.into();

        if let Some(location) = self.locations.get(&symbol).copied() {
            // Symbol already assigned a location
            self.values[location] = new_value;
        } else {
            // Assign a new location for the symbol and set it to the value
            let offset = self.values.len();
            self.values.push(new_value);

            self.locations.insert(symbol.into(), offset);
        }
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
    pub fn value_for_symbol(&mut self, symbol: impl Into<TalkSymbol>) -> Option<&mut TValue> {
        self.location_for_symbol(symbol).map(|location| self.at_location(location))
    }
}

impl TalkValueStore<TalkValue> {
    ///
    /// Calls remove_reference on all the values in this context, leaving it empty
    ///
    pub fn remove_all_references(&mut self, context: &mut TalkContext) {
        for val in self.values.iter() {
            val.remove_reference(context);
        }

        self.values     = vec![];
        self.locations  = HashMap::new();
    }
}

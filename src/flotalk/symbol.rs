use std::collections::{HashMap};
use std::fmt;
use std::sync::*;

lazy_static! {
    static ref SYMBOL_VALUES: Mutex<HashMap<&'static str, TalkSymbol>>  = Mutex::new(HashMap::new());
    static ref SYMBOL_NAMES: Mutex<HashMap<TalkSymbol, &'static str>>   = Mutex::new(HashMap::new());
    static ref NEXT_SYMBOL_ID: Mutex<usize>                             = Mutex::new(0);
}

///
/// A unique identifier for a FloTalk symbol
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TalkSymbol(usize);

impl<'a> From<&'a str> for TalkSymbol {
    fn from(val: &'a str) -> TalkSymbol {
        let mut values = SYMBOL_VALUES.lock().unwrap();

        if let Some(symbol) = values.get(val) {
            // Symbol already stored
            *symbol
        } else {
            // Create a new symbol value
            let symbol_id = { 
                let mut next_symbol_id  = NEXT_SYMBOL_ID.lock().unwrap();
                let symbol_id           = *next_symbol_id;
                *next_symbol_id         += 1;

                symbol_id
            };

            // Allocate the symbol ID
            let new_symbol = TalkSymbol(symbol_id);

            // Convert to a string (symbol name mappings are kept for the life of the program)
            let static_name = String::from(val).into_boxed_str();
            let static_name = Box::leak(static_name);

            // Store the new symbol
            values.insert(static_name, new_symbol);
            SYMBOL_NAMES.lock().unwrap().insert(new_symbol, static_name);

            new_symbol
        }
    }
}

impl TalkSymbol {
    ///
    /// Returns the name of this symbol
    ///
    pub fn name(&self) -> &'static str {
        *(SYMBOL_NAMES.lock().unwrap().get(self).unwrap())
    }

    ///
    /// The ID number for this symbol
    ///
    pub fn id(&self) -> usize {
        self.0
    }
}

impl fmt::Debug for TalkSymbol {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.write_fmt(format_args!("#'{}'", self.name()))
    }
}

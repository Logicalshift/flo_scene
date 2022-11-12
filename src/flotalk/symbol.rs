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

impl<'a> From<&'a TalkSymbol> for TalkSymbol {
    #[inline]
    fn from(val: &'a TalkSymbol) -> TalkSymbol {
        *val
    }
}

impl<'a> From<&'a String> for TalkSymbol {
    #[inline]
    fn from(val: &'a String) -> TalkSymbol {
        TalkSymbol::from(val.as_str())
    }
}

impl From<String> for TalkSymbol {
    #[inline]
    fn from(val: String) -> TalkSymbol {
        TalkSymbol::from(val.as_str())
    }
}

impl From<Arc<String>> for TalkSymbol {
    #[inline]
    fn from(val: Arc<String>) -> TalkSymbol {
        TalkSymbol::from(&*val)
    }
}

impl<'a> From<&'a Arc<String>> for TalkSymbol {
    #[inline]
    fn from(val: &'a Arc<String>) -> TalkSymbol {
        TalkSymbol::from(&**val)
    }
}

impl TalkSymbol {
    ///
    /// Creates an 'unnamed' symbol, which cannot be returned by `TalkSymbol::from()`
    ///
    /// If you retrieve the name of an unnamed symbol, it will be something like ` <UNNAMED#x> `. Spaces are usually
    /// not allowed in symbol names, but note that `TalkSymbol::from(" <UNNAMED#x> ")` will not return the same symbol!
    ///
    pub fn new_unnamed() -> TalkSymbol {
        // Create an ID for our unnamed symbol
        let symbol_id = { 
            let mut next_symbol_id  = NEXT_SYMBOL_ID.lock().unwrap();
            let symbol_id           = *next_symbol_id;
            *next_symbol_id         += 1;

            symbol_id
        };

        let symbol = TalkSymbol(symbol_id);

        // Create a fake name so that name() won't panic (but it has no mapping the other way)
        let fake_name = format!(" <UNNAMED#{}> ", symbol_id);
        let fake_name = fake_name.into_boxed_str();
        let fake_name = Box::leak(fake_name);

        SYMBOL_NAMES.lock().unwrap().insert(symbol, fake_name);

        symbol
    }

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

    ///
    /// True if this is a keyword symbol (eg: `foo:`, or false if it's not (eg: 'foo'))
    ///
    pub fn is_keyword(&self) -> bool {
        self.name().chars().last() == Some(':')
    }
}

impl fmt::Debug for TalkSymbol {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.write_fmt(format_args!("#'{}'", self.name()))
    }
}

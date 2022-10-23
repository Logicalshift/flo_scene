//!
//! # The flotalk scripting language
//!
//! `flotalk` is a scripting language for `flo_scene`, based on SmallTalk-80.
//!

mod instruction;
mod program;
mod parser;
mod location;
mod parse_error;
mod pushback_stream;
mod symbol;
mod context;
mod class;

pub use self::instruction::*;
pub use self::program::*;
pub use self::parser::*;
pub use self::symbol::*;
pub use self::context::*;
pub use self::class::*;

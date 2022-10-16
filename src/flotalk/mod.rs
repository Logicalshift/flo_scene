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

pub use self::instruction::*;
pub use self::program::*;
pub use self::parser::*;

//!
//! # The flotalk scripting language
//!
//! `flotalk` is a scripting language for `flo_scene`, based on SmallTalk-80.
//!

mod instruction;
mod expression;
mod parser;
mod location;
mod parse_error;
mod pushback_stream;
mod symbol;
mod context;
mod class;
mod reference;
mod message;
mod continuation;
mod runtime;
mod error;
mod simple_evaluator;
mod simple_evaluator_block;
mod allocator;
mod class_builder;
mod value;
mod value_store;
mod value_messages;

pub use self::instruction::*;
pub use self::expression::*;
pub use self::parser::*;
pub use self::symbol::*;
pub use self::context::*;
pub use self::class::*;
pub use self::reference::*;
pub use self::message::*;
pub use self::continuation::*;
pub use self::value::*;
pub use self::runtime::*;
pub use self::error::*;
pub use self::value_store::*;
pub use self::simple_evaluator::*;
pub use self::simple_evaluator_block::*;
pub use self::allocator::*;
pub use self::class_builder::*;
pub use self::value_messages::*;

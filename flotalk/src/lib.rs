//!
//! # The flotalk scripting language
//!
//! `flotalk` is a scripting language for `flo_scene`, based on SmallTalk-80.
//!

#[allow(unused_imports)]
#[macro_use] extern crate flo_talk_macros;

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
mod message_converters;
mod continuation;
mod runtime;
mod error;
mod simple_evaluator;
mod allocator;
mod class_builder;
mod number;
mod value;
mod value_messages;
mod dispatch_table;
mod releasable;
mod standard_classes;
mod read_write_queue;
mod symbol_table;

pub mod sparse_array;

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
pub use self::simple_evaluator::*;
pub use self::allocator::*;
pub use self::class_builder::*;
pub use self::value_messages::*;
pub use self::dispatch_table::*;
pub use self::number::*;
pub use self::releasable::*;
pub use self::standard_classes::*;
pub use self::read_write_queue::*;
pub use self::symbol_table::*;

#[doc(hidden)] pub use flo_talk_macros::*;
#[doc(hidden)] pub use once_cell;
#[doc(hidden)] pub use smallvec;

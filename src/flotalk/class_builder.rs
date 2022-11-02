use super::allocator::*;
use super::class::*;
use super::context::*;
use super::message::*;
use super::value::*;

use std::collections::{HashMap};

///
/// The class builder can be used to build a FloTalk class quickly from Rust
///
pub struct TalkClassBuilder<TDataType> {
    /// Supported instance messages
    instance_messages:  HashMap<TalkMessageSignatureId, Box<dyn Send + Sync + Fn(&mut TalkContext, TalkMessage, &mut TDataType) -> TalkValue>>,

    /// Supported class messages
    class_messages:     HashMap<TalkMessageSignatureId, Box<dyn Send + Sync + Fn(&mut TalkStandardAllocator<TDataType>, TalkMessage) -> TalkValue>>,
}


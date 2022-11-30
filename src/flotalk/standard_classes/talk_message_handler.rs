use crate::flotalk::class::*;
use crate::flotalk::continuation::*;
use crate::flotalk::context::*;
use crate::flotalk::reference::*;
use crate::flotalk::releasable::*;
use crate::flotalk::symbol_table::*;
use crate::flotalk::value::*;

use smallvec::*;

use std::sync::*;

///
/// Represents a function that handles a message sent to a class message handler: it's possible to retrieve these from blocks
///
/// These are defined with the arguments for the message plus a 'super' value pointing at the superclass
///
pub struct TalkClassMessageHandler {
    /// The number of arguments expected by the message handler
    pub (super) expected_args: usize,

    /// message_handler(class_id, arguments, super, context)
    pub (super) message_handler: Box<dyn Send + Sync + for<'a> Fn(TalkClass, TalkOwned<'a, SmallVec<[TalkValue; 4]>>, TalkOwned<'a, TalkValue>, &'a TalkContext) -> TalkContinuation<'static>>
}

///
/// Represents a function that handles a message sent to a class message handler
///
/// These are defined with the arguments for the message plus the instance variable cell block. The function returned is a function that binds the symbol table
/// for the instance variables.
///
pub struct TalkInstanceMessageHandler {
    /// The number of arguments expected by the message handler
    pub (super) expected_args: usize,

    /// Binds a symbol table to this block (names the cells in the instance cell block)
    pub (super) bind_message_handler: Box<dyn Send + FnOnce(Arc<Mutex<TalkSymbolTable>>) ->
        Box<dyn Send + Sync + for<'a> Fn(TalkClass, TalkOwned<'a, SmallVec<[TalkValue; 4]>>, TalkOwned<'a, TalkCellBlock>, &'a TalkContext) -> TalkContinuation<'static>>>
}

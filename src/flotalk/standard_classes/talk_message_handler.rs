use crate::flotalk::dispatch_table::*;
use crate::flotalk::message::*;
use crate::flotalk::reference::*;
use crate::flotalk::symbol_table::*;

use std::sync::*;

///
/// Represents a function that handles a message sent to a class message handler: it's possible to retrieve these from blocks
///
/// These are defined with the arguments for the message plus a 'super' value pointing at the superclass
///
pub struct TalkClassMessageHandler {
    /// Defines this instance message in a dispatch table. The 'self' type is expected to be a reference cell (ie, the data handle should be a reference to a cell block
    /// with the instance variables in it)
    pub (super) define_in_dispatch_table: Box<dyn Send + FnOnce(&mut TalkMessageDispatchTable<()>, TalkMessageSignatureId, Option<TalkReference>) -> ()>,
}

///
/// Represents a function that handles a message sent to a class message handler
///
/// These are defined with the arguments for the message plus the instance variable cell block. The function returned is a function that binds the symbol table
/// for the instance variables.
///
pub struct TalkInstanceMessageHandler {
    /// Defines this instance message in a dispatch table. The 'self' type is expected to be a reference cell (ie, the data handle should be a reference to a cell block
    /// with the instance variables in it)
    pub (super) define_in_dispatch_table: Box<dyn Send + FnOnce(&mut TalkMessageDispatchTable<TalkReference>, TalkMessageSignatureId, Arc<Mutex<TalkSymbolTable>>) -> ()>,
}

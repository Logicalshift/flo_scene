use super::reference::*;
use super::value::*;

use futures::task::{Poll, Context};

///
/// Raw functions return a continuation, which specifies how a result may be retrieved
///
pub enum TalkContinuation {
    /// A value that's ready now
    Ready(TalkValue),

    /// A value that is ready when a future completes
    Later(Box<dyn Fn(TalkReference, &mut Context) -> Poll<TalkValue>>),
}

use super::value::*;

use futures::future::{BoxFuture};

///
/// Raw functions return a continuation, which specifies how a result may be retrieved
///
pub enum TalkContinuation {
    /// A value that's ready now
    Ready(TalkValue),

    /// A value that is ready when a future completes
    Later(BoxFuture<'static, TalkValue>),
}

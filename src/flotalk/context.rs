use ::desync::*;

use std::sync::*;

///
/// A talk context is a self-contained representation of the state of a flotalk interpreter
///
/// Contexts are only accessed on one thread at a time.
///
pub struct TalkContext {

}

impl TalkContext {
    ///
    /// Creates a new, empty context
    ///
    pub fn empty() -> Arc<Desync<TalkContext>> {
        let context = TalkContext {

        };

        Arc::new(Desync::new(context))
    }
}

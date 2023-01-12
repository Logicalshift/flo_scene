///
/// Context that is local to a single TalkContinuation
///
/// `TalkContext` is a type that's used to represent the shared persistent state of a FloTalk runtime. This represents the
/// ephemeral state of a continuation, which is lost once the continuation completes. Its principle use is in combination with
/// the `Inverted` class, where it can be used to store targets for block messages.
///
#[derive(Clone)]
pub struct TalkLocalContext {

}

impl Default for TalkLocalContext {
    fn default() -> Self {
        TalkLocalContext {
            
        }
    }
}
use futures::prelude::*;
use futures::channel::mpsc;

///
/// The scene context is a per-subprogram way to access output streams
///
/// The context is passed to the program when it starts, and can also be retrieved from any code executing as part of that subprogram.
///
pub struct SceneContext {

}

impl SceneContext {
    ///
    /// Retrieves a stream for sending messages of the specified type
    ///
    /// If no receiver is attached to this stream type for this program, the 
    ///
    pub fn send<TMessageType>(&self) -> impl Sink<TMessageType>
    where
        TMessageType: 'static + Send + Sync,
    {
        todo!();

        let (send, _recv ) = mpsc::channel(1);
        send
    }
}

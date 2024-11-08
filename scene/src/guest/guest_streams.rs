use crate::host::scene_message::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

///
/// Identifier for a stream that is available across a guest interface
///
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct GuestStreamId(pub usize);

///
/// Trait implemented by objects that can manage guest streams: these are streams whose data is sent across a guest API. This
/// provides a way to send data using a native stream across a guest interface.
///
pub trait GuestStreamManager {
    ///
    /// Creates a guest stream that will send messages to the other side of the guest interface
    ///
    /// The returned stream ID can be used to receive the messages on the other side.
    ///
    fn send<TMessage: SceneMessage>(stream: impl Stream<Item=TMessage>) -> GuestStreamId;

    ///
    /// Receives the content of a stream from the other side of the guest interface
    ///
    fn recv<TMessage: SceneMessage>(stream_id: GuestStreamId) -> BoxStream<'static, TMessage>;
}

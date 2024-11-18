use crate::host::error::*;
use crate::host::serialization_context::*;

use futures::stream::{BoxStream};

///
/// Serialization context used for guest subprograms
///
#[derive(Clone)]
pub (super) struct GuestSerializationContext {

}

impl GuestSerializationContext {
    ///
    /// Creates a new serialization context for this guest
    ///
    pub fn new() -> Self {
        GuestSerializationContext { }
    }
}

impl SerializationContext for GuestSerializationContext {
    fn send_stream(&self, stream: BoxStream<'static, Vec<u8>>) -> Result<SerializationId, SceneSendError<BoxStream<'static, Vec<u8>>>> {
        todo!()
    }

    fn receive_stream(&self, stream_id: SerializationId) -> Result<BoxStream<'static, Vec<u8>>, SceneSendError<SerializationId>> {
        todo!()
    }

    fn send_function(&self, callback: RemoteCallbackFn) -> Result<SerializationId, SceneSendError<RemoteCallbackFn>> {
        todo!()
    }

    fn receive_function(&self, callback_id: SerializationId) -> Result<RemoteCallbackFn, SceneSendError<SerializationId>> {
        todo!()
    }
}

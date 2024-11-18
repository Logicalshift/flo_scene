use crate::host::serialization_context::*;

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
    fn send_stream(&self, stream: futures::stream::BoxStream<'static, Vec<u8>>) -> Result<SerializationId, crate::SceneSendError<futures::stream::BoxStream<'static, Vec<u8>>>> {
        todo!()
    }

    fn receive_stream(&self, stream_id: SerializationId) -> Result<futures::stream::BoxStream<'static, Vec<u8>>, crate::SceneSendError<SerializationId>> {
        todo!()
    }

    fn send_function(&self, callback: RemoteCallbackFn) -> Result<SerializationId, crate::SceneSendError<RemoteCallbackFn>> {
        todo!()
    }

    fn receive_function(&self, callback_id: SerializationId) -> Result<RemoteCallbackFn, crate::SceneSendError<SerializationId>> {
        todo!()
    }
}

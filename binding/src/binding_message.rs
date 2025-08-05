use flo_binding::*;
use flo_scene::*;
use flo_scene::postcard;

use futures::prelude::*;
use serde::*;
use serde::de::{Error as DeError};
use serde::ser::{Error as SeError};

///
/// A binding message is used to pass bindings between components. It contains a binding, but works
/// similarly to a query response in many ways.
///
/// Bindings can be converted to and from streams, so a binding message is effectively a stream of values
/// for the binding. This can also be used to design message types that work with bindings but which can
/// also be used with the command processor (via the to/from guest message functions).
///
/// For a binding value to be sent as a message this way it must itself be a SceneMessage.
///
#[derive(Clone)]
pub struct BindingMessage<TValue>(pub BindRef<TValue>);

impl<TValue> Serialize for BindingMessage<TValue> {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        Err(S::Error::custom("BindingMessage cannot be serialized"))
    }
}

impl<'a, TValue> Deserialize<'a> for BindingMessage<TValue> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a> 
    {
        Err(D::Error::custom("BindingMessage cannot be serialized"))
    }
}

/// Serialization structure for a binding message
#[derive(Serialize, Deserialize)]
struct SerializedBindingMessage(SerializationId);

impl<TValue> SceneMessage for BindingMessage<TValue>
where 
    TValue: 'static + Default + Clone + PartialEq + SceneMessage,
{
    fn initialise(_init_context: &impl flo_scene::SceneInitialisationContext) { }

    fn message_type_name() -> String { format!("flo_scene_binding::BindingMessage<{:?}>", TValue::message_type_name()) }

    fn serializable() -> bool { false }

    fn to_guest_message(self, context: &impl flo_scene::SerializationContext) -> Result<Vec<u8>, flo_scene::SceneSendError<Self>> {
        // Create a serialized stream of messages from the stream, and use the context to pass it to the guest
        let BindingMessage(binding)     = self;
        let binding_stream              = follow(binding.clone());
        let serialized_stream           = binding_stream.flat_map(|val| stream::iter(postcard::to_stdvec(&val)));
        let serialized_stream           = context.send_stream(serialized_stream.boxed()).map_err(|err| err.map(|_| BindingMessage(binding.clone())))?;

        // Serialize the response itself
        let serialized_response         = SerializedBindingMessage(serialized_stream);
        let serialized_response         = postcard::to_stdvec(&serialized_response).map_err(|err| SceneSendError::CannotSerialize(BindingMessage(binding.clone()), format!("{:?}", err)))?;

        // Created a guest message
        Ok(serialized_response)
    }

    fn from_guest_message(value: &Vec<u8>, context: &impl flo_scene::SerializationContext) -> Result<Self, flo_scene::SceneSendError<()>> {
        // Deserialize as a serailized binding message
        let serialized_stream = postcard::from_bytes::<SerializedBindingMessage>(value)
            .map_err(move |postcard_error| SceneSendError::CannotDeserialize((), format!("{:?}", postcard_error)))?;

        // Receive the stream from the guest side
        let stream = context.receive_stream(serialized_stream.0).map_err(|err| err.map(|_| ()))?;

        // Deserialize it
        let stream = stream.flat_map(|msg| stream::iter(postcard::from_bytes(&msg)));

        // Return the resulting stream
        Ok(BindingMessage(bind_stream(stream, TValue::default(), |_, val| val).into()))
    }
}

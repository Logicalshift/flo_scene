use crate::entity_id::*;

use futures::stream::{BoxStream};

use std::any::{TypeId, Any};
use std::sync::*;

use futures::prelude::*;
use futures::task;
use futures::task::{Poll, Context};
use std::pin::*;

#[cfg(feature="properties")] use crate::context::*;
#[cfg(feature="properties")] use crate::error::*;
#[cfg(feature="properties")] use crate::entity_channel::*;
#[cfg(feature="properties")] use crate::stream_entity_response_style::*;

#[cfg(feature="properties")] use std::collections::{HashMap};

#[cfg(feature="properties")] 
lazy_static! {
    static ref MESSAGE_PROCESSORS: RwLock<HashMap<TypeId, Box<dyn Send + Sync + Fn(Box<dyn Send + Any>, &PropertiesState) -> ()>>> = RwLock::new(HashMap::new());
}

// TODO: we can also use BoxedEntityChannel<'static, TValue, ()> as a sink, which might be more consistent, but not sure how to get the proper behaviour
// for dropping intermediate values reliably.

///
/// Core data shared between a property sink and a property stream
///
struct PropertyStreamCore<TValue> {
    is_closed:  bool,
    next_value: Option<TValue>,
    waker:      Option<task::Waker>,
}

///
/// Sink used for sending property values
///
/// This has the property that if the last value was not read before a new value is received, the new value
/// will replace it. This is suitable for properties, which have a 'current' value and where intermediate values
/// are considered outdated.
///
pub struct PropertySink<TValue> {
    core: Arc<Mutex<PropertyStreamCore<TValue>>>,
}

///
/// Receiver for values sent by the property sink
///
pub struct PropertyStream<TValue> {
    core: Arc<Mutex<PropertyStreamCore<TValue>>>,
}

///
/// A single value property is defined in a format that's suitable for use with the `flo_binding` library. That is, as a stream of
/// values. It won't be fully defined until the stream returns an initial value.
///
/// The `follow()` function in the `flo_binding` crate is the easiest way to generate a suitable stream. Typically a property stream
/// will drop values that are not read in time: the `ExpiringPublisher` publisher found in the `flo_stream` crate is a good way to 
/// generate these streams from other sources.
///
/// Note that while there's a standard property entity with the `PROPERTIES` entity ID, it's possible to create new property entities
/// to define properties with entirely independent 'namespaces'.
///
pub struct PropertyDefinition<TValue> 
where
    TValue: 'static + Send + Sized,
{
    /// The entity that owns this property
    pub owner: EntityId,

    /// The name of this property
    pub name: Arc<String>,

    /// The stream of values for this property
    ///
    /// The property won't be created until this has returned at least one value. The property will stop updating but not be destroyed
    /// if this stream is closed.
    pub values: BoxStream<'static, TValue>,
}

///
/// A reference to an existing property
///
pub struct PropertyReference {
    /// The entity that owns this property
    pub owner: EntityId,

    /// The name of the property
    pub name: Arc<String>,
}

///
/// Requests that can be made of a property entity
///
pub enum PropertyRequest<TValue> 
where
    TValue: 'static + Send + Sized,
{
    /// Creates a new property
    CreateProperty(PropertyDefinition<TValue>),

    /// Removes the property with the specified name
    DestroyProperty(PropertyReference),

    /// Sends changes to the property to the specified property sink. The value will be 'None' when the property is destroyed.
    Follow(PropertyReference, PropertySink<Option<TValue>>),
}

///
/// An internal property request contains an 'Any' request for properties of a given type
///
enum InternalPropertyRequest {
    /// A PropertyRequest<x> that's wrapped in a Box<Any> for a type that is recognised by the property entity
    AnyRequest(Box<dyn Send + Any>),
}

impl<TValue> From<PropertyRequest<TValue>> for InternalPropertyRequest 
where
    TValue: 'static + Send
{
    fn from(req: PropertyRequest<TValue>) -> InternalPropertyRequest {
        // The internal value is Option<PropertyRequest<TValue>>, which allows the caller to take the value out of the box later on
        InternalPropertyRequest::AnyRequest(Box::new(Some(req)))
    }
}

impl<TValue> Sink<TValue> for PropertySink<TValue> {
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Always ready
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: TValue) -> Result<(), Self::Error> {
        let waker = {
            let mut core    = self.core.lock().unwrap();

            // We always replace the next value (as this is a stream of states, so any previous state is now outdated)
            core.next_value = Some(item);

            // Take the waker in order to wake up the stream, if it's asleep
            core.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Always flushed
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // We just close straight away
        let waker = {
            let mut core    = self.core.lock().unwrap();
            core.is_closed  = true;

            // Take the waker in order to wake up the stream, if it's asleep
            core.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }

        Poll::Ready(Ok(()))
    }
}

impl<TValue> Stream for PropertyStream<TValue> {
    type Item = TValue;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<TValue>> {
        let mut core = self.core.lock().unwrap();

        if core.is_closed {
            // Result is 'None' if the sink has been closed
            Poll::Ready(None)
        } else if let Some(value) = core.next_value.take() {
            // Return the value if one is ready
            Poll::Ready(Some(value))
        } else {
            // Update the waker
            core.waker = Some(context.waker().clone());

            // Wait for the value to become available
            Poll::Pending
        }
    }
}

///
/// Creates a stream and a sink suitable for sending property values
///
/// Property values are considered states, and the stream is a stream of the 'latest state' updates for a property only. Ie, if
/// two values are sent to the property sink, but the stream is only read after the second value is sent, the stream will only
/// contain the second value.
///
pub fn property_stream<TValue>() -> (PropertySink<TValue>, PropertyStream<TValue>) {
    let core    = PropertyStreamCore {
        is_closed:  false,
        next_value: None,
        waker:      None,
    };
    let core    = Arc::new(Mutex::new(core));

    let sink    = PropertySink { core: Arc::clone(&core) };
    let stream  = PropertyStream { core: Arc::clone(&core) };

    (sink, stream)
}

///
/// Retrieves an entity channel to talk to the properties entity about properties of type `TValue`. This is the same as calling `context.send_to()`
/// except this will ensure a suitable conversion for communicating with the properties entity is set up. That is `send_to()` won't work until this
/// has been called at least once for the scene with the property type.
///
/// Typically `entity_id` should be `PROPERTIES` here, but it's possible to run multiple sets of properties in a given scene so other values are
/// possible if `create_properties_entity()` has been called for other entity IDs.
///
pub fn properties_channel<TValue>(entity_id: EntityId, context: &Arc<SceneContext>) -> Result<BoxedEntityChannel<'static, PropertyRequest<TValue>, ()>, EntityChannelError>
where
    TValue: 'static + Send + Sized
{
    // Ensure that the message is converted to an internal request
    context.convert_message::<PropertyRequest<TValue>, InternalPropertyRequest>()?;

    // Send messages to the properties entity
    context.send_to(entity_id)
}

///
/// Used to represent the state of the properties entity at any given time
///
struct PropertiesState {
    
}

///
/// Creates a new properties entity with the specified ID in the given context
///
/// The result here is '()' as the properties channel is defined per property type. Call `properties_channel()` to retrieve channels
/// for properties of particular types. Note that while calling `send_to()` on a scene context will also often work, it won't
/// automatically set up the needed type conversion, so it will fail if called for a type that hasn't been encountered before.
///
#[cfg(feature="properties")]
pub fn create_properties_entity(entity_id: EntityId, context: &Arc<SceneContext>) -> Result<(), CreateEntityError> {
    // Create the state for the properties entity
    let mut state = PropertiesState {

    };

    // Create the entity itself
    context.create_stream_entity(entity_id, StreamEntityResponseStyle::default(), move |_context, mut messages| async move {
        while let Some(message) = messages.next().await {
            let message: InternalPropertyRequest = message;

            match message {
                InternalPropertyRequest::AnyRequest(request) => {
                    // Lock the message processors so we can read from them
                    let message_processors = MESSAGE_PROCESSORS.read().unwrap();

                    // Fetch the ID of the type in the request
                    let request_type = (&*request).type_id();

                    // Try to retrieve a processor for this type (these are created when properties_channel is called to retrieve properties of this type)
                    if let Some(request_processor) = message_processors.get(&request_type) {
                        // Process the request
                        request_processor(request, &mut state);
                    }
                }
            }
        }
    })?;

    Ok(())
}

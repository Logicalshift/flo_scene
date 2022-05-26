use crate::entity_id::*;

use futures::stream::{BoxStream};

use std::any::{Any};
use std::sync::*;
use std::marker::{PhantomData};

#[cfg(feature="properties")] use crate::context::*;
#[cfg(feature="properties")] use crate::error::*;
#[cfg(feature="properties")] use crate::entity_channel::*;
#[cfg(feature="properties")] use crate::simple_entity_channel::*;
#[cfg(feature="properties")] use crate::stream_entity_response_style::*;

#[cfg(feature="properties")] use futures::prelude::*;
#[cfg(feature="properties")] use std::sync::*;

// TODO: we can also use BoxedEntityChannel<'static, TValue, ()> as a sink, which might be more consistent, but not sure how to get the proper behaviour
// for dropping intermediate values reliably.

///
/// Sink used for sending property values
///
/// This has the property that if the last value was not read before a new value is received, the new value
/// will replace it. This is suitable for properties, which have a 'current' value and where intermediate values
/// are considered outdated.
///
pub struct PropertySink<TValue> {
    x: PhantomData<TValue>,
    // (TODO)
}

///
/// Receiver for values sent by the property sink
///
pub struct PropertyStream<TValue> {
    x: PhantomData<TValue>,
    // (TODO)
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

    /// Sends changes to the property to the specified entity channel. The value will be 'None' when the property is destroyed.
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
        InternalPropertyRequest::AnyRequest(Box::new(req))
    }
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

    // Create the entity itself
    context.create_stream_entity(entity_id, StreamEntityResponseStyle::default(), move |_context, mut messages| async move {
        while let Some(message) = messages.next().await {
            let message: InternalPropertyRequest = message;

            // TODO
        }
    })?;

    Ok(())
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

use crate::entity_id::*;

use futures::stream::{BoxStream};

use std::any::{Any};
use std::sync::*;
use std::marker::{PhantomData};

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

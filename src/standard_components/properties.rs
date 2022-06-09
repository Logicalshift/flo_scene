use crate::entity_id::*;
use crate::context::*;
use crate::error::*;
use crate::entity_channel::*;
use crate::message::*;

use super::entity_registry::*;
use super::entity_ids::*;

use flo_binding::*;

use futures::prelude::*;

use std::any::{TypeId, Any};
use std::sync::*;
use std::collections::{HashMap};

#[cfg(feature="properties")] 
lazy_static! {
    static ref MESSAGE_PROCESSORS: RwLock<HashMap<TypeId, Box<dyn Send + Sync + Fn(Box<dyn Send + Any>, &mut PropertiesState, &Arc<SceneContext>) -> Option<InternalPropertyResponse>>>> = RwLock::new(HashMap::new());
}

// TODO: we can also use BoxedEntityChannel<'static, TValue, ()> as a sink, which might be more consistent, but not sure how to get the proper behaviour
// for dropping intermediate values reliably.

///
/// Some property requests can respond with a value (eg: a binding)
///
struct InternalPropertyResponse(Box<dyn Send + Any>);

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
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    /// The entity that owns this property
    pub owner: EntityId,

    /// The name of this property
    pub name: Arc<String>,

    /// The stream of values for this property
    ///
    /// The property won't be created until this has returned at least one value. The property will stop updating but not be destroyed
    /// if this stream is closed.
    pub values: BindRef<TValue>,
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
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    /// Creates a new property
    CreateProperty(PropertyDefinition<TValue>),

    /// Removes the property with the specified name
    DestroyProperty(PropertyReference),

    /// Retrieves the `BindRef<TValue>` containing this property (this shares the data more efficiently than Follow does)
    Get(PropertyReference),
}

///
/// An internal property request contains an 'Any' request for properties of a given type
///
enum InternalPropertyRequest {
    /// A PropertyRequest<x> that's wrapped in a Box<Any> for a type that is recognised by the property entity
    AnyRequest(Box<dyn Send + Any>),

    /// Pings the properties entity to ensure it's ready for requests
    Ready,

    /// A new entity was created
    CreatedEntity(EntityId),

    /// An entity was destroyed
    DestroyedEntity(EntityId),
}

impl<TValue> From<PropertyRequest<TValue>> for InternalPropertyRequest 
where
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    fn from(req: PropertyRequest<TValue>) -> InternalPropertyRequest {
        // The internal value is Option<PropertyRequest<TValue>>, which allows the caller to take the value out of the box later on
        InternalPropertyRequest::AnyRequest(Box::new(Some(req)))
    }
}

impl From<EntityUpdate> for InternalPropertyRequest {
    fn from(req: EntityUpdate) -> InternalPropertyRequest {
        match req {
            EntityUpdate::CreatedEntity(entity_id)      => InternalPropertyRequest::CreatedEntity(entity_id),
            EntityUpdate::DestroyedEntity(entity_id)    => InternalPropertyRequest::DestroyedEntity(entity_id),
        }
    }
}

impl<TValue> PropertyDefinition<TValue>
where
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    ///
    /// Creates a new property definition that has the most recent value received on a stream
    ///
    pub fn from_stream(owner: EntityId, name: &str, values: impl 'static + Send + Unpin + Stream<Item=TValue>, default_value: TValue) -> PropertyDefinition<TValue> {
        PropertyDefinition {
            owner:  owner,
            name:   Arc::new(name.to_string()),
            values: BindRef::from(bind_stream(values, default_value, |_old, new| new)),
        }
    }

    ///
    /// Creates a new property definition from an existing bound value
    ///
    pub fn from_binding(owner: EntityId, name: &str, values: impl Into<BindRef<TValue>>) -> PropertyDefinition<TValue> {
        PropertyDefinition {
            owner:  owner,
            name:   Arc::new(name.to_string()),
            values: values.into(),
        }
    }
}

impl PropertyReference {
    ///
    /// Creates a new property definition
    ///
    pub fn new(owner: EntityId, name: &str) -> PropertyReference {
        PropertyReference {
            owner:  owner,
            name:   Arc::new(name.to_string()),
        }
    }
}

///
/// Retrieves an entity channel to talk to the properties entity about properties of type `TValue`. This is the same as calling `context.send_to()`
/// except this will ensure a suitable conversion for communicating with the properties entity is set up. That is `send_to()` won't work until this
/// has been called at least once for the scene with the property type.
///
/// Typically `entity_id` should be `PROPERTIES` here, but it's possible to run multiple sets of properties in a given scene so other values are
/// possible if `create_properties_entity()` has been called for other entity IDs.
///
pub async fn properties_channel<TValue>(entity_id: EntityId, context: &Arc<SceneContext>) -> Result<BoxedEntityChannel<'static, PropertyRequest<TValue>, Option<BindRef<TValue>>>, EntityChannelError>
where
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    // Add a processor for this type if one doesn't already exist
    {
        let mut message_processors = MESSAGE_PROCESSORS.write().unwrap();

        message_processors.entry(TypeId::of::<Option<PropertyRequest<TValue>>>()).or_insert_with(|| {
            Box::new(|message, state, context| {
                let response = process_message::<TValue>(message, state, context);
                let response = if let Some(response) = response {
                    Some(InternalPropertyResponse(Box::new(Some(response))))
                } else {
                    None
                };

                response
            })
        });
    }

    // Before returning a channel, wait for the properties entity to become ready
    // (This is most useful at startup when we need the entity tracking to start up before anything else)
    context.send::<_, ()>(PROPERTIES, InternalPropertyRequest::Ready).await.ok();

    // Ensure that the message is converted to an internal request
    context.convert_message::<PropertyRequest<TValue>, InternalPropertyRequest>()?;
    context.map_response::<Option<InternalPropertyResponse>, Option<BindRef<TValue>>, _>(|internal_response| {
        if let Some(InternalPropertyResponse(mut any_box)) = internal_response {
            // The 'Any' inside InternalPropertyResponse is itself an option (so we can extract the value)
            if let Some(optional_bindref) = any_box.downcast_mut::<Option<BindRef<TValue>>>() {
                // Take the response to de-any-fy it
                optional_bindref.take()
            } else {
                // Wrong type of response
                None
            }
        } else {
            None
        }
    })?;

    // Send messages to the properties entity
    context.send_to(entity_id)
}

///
/// Used to represent the state of the properties entity at any given time
///
struct PropertiesState {
    /// The properties for each entity in the scene. The value is an `Arc<Mutex<Property<TValue>>>` in an any box
    properties: HashMap<EntityId, HashMap<Arc<String>, Box<dyn Send + Any>>>,
}

///
/// Data associated with a property
///
struct Property<TValue> {
    /// The current value, if known
    current_value: BindRef<TValue>,
}

///
/// Processes a message, where the message is expected to be of a particular type
///
fn process_message<TValue>(any_message: Box<dyn Send + Any>, state: &mut PropertiesState, _context: &Arc<SceneContext>) -> Option<BindRef<TValue>>
where
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    // Try to unbox the message. The type is Option<PropertyRequest> so we can take it out of the 'Any' reference
    let mut any_message = any_message;
    let message         = any_message.downcast_mut::<Option<PropertyRequest<TValue>>>().and_then(|msg| msg.take());
    let message         = if let Some(message) = message { message } else { return None; };

    // The action depends on the message content
    use PropertyRequest::*;
    match message {
        CreateProperty(definition) => { 
            let owner   = definition.owner;
            let name    = definition.name;
            let values  = definition.values;

            // Create the property
            let property                        = Property::<TValue> {
                current_value:  values,
            };
            let property                        = Arc::new(Mutex::new(property));

            // Store a copy of the property in the state (we use the entity registry to know which entities exist)
            if let Some(entity_properties) = state.properties.get_mut(&owner) {
                entity_properties.insert(name, Box::new(Arc::clone(&property)));
            }

            None
        }

        DestroyProperty(reference) => {
            if let Some(entity_properties) = state.properties.get_mut(&reference.owner) {
                entity_properties.remove(&reference.name);
            }

            None
        }

        Get(reference) => {
            // See if there's a property with the appropriate name
            if let Some(property) = state.properties.get_mut(&reference.owner).and_then(|entity| entity.get_mut(&reference.name)) {
                if let Some(property) = property.downcast_mut::<Arc<Mutex<Property<TValue>>>>() {
                    // Return the binding to the caller
                    Some(property.lock().unwrap().current_value.clone())
                } else {
                    None
                }
            } else {
                None
            }
        }
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
    let mut state   = PropertiesState {
        properties: HashMap::new()
    };

    context.convert_message::<EntityUpdate, InternalPropertyRequest>().unwrap();
    context.map_response::<Option<InternalPropertyResponse>, (), _>(|_| ()).unwrap();

    // Create the entity itself
    context.create_entity(entity_id, move |context, mut messages| async move {
        // Request updates from the entity registry
        let properties      = context.send_to::<EntityUpdate, ()>(entity_id).unwrap();
        if let Some(mut entity_registry) = context.send_to::<_, ()>(ENTITY_REGISTRY).ok() {
            entity_registry.send(EntityRegistryRequest::TrackEntities(properties)).await.ok();
        }

        while let Some(message) = messages.next().await {
            let message: Message<InternalPropertyRequest, Option<InternalPropertyResponse>> = message;
            let (message, responder) = message.take();

            match message {
                InternalPropertyRequest::Ready => {
                    // This is just used to synchronise requests to the entity
                    responder.send(None).ok();
                }

                InternalPropertyRequest::AnyRequest(request) => {
                    // Lock the message processors so we can read from them
                    let message_processors = MESSAGE_PROCESSORS.read().unwrap();

                    // Fetch the ID of the type in the request
                    let request_type = (&*request).type_id();

                    // Try to retrieve a processor for this type (these are created when properties_channel is called to retrieve properties of this type)
                    if let Some(request_processor) = message_processors.get(&request_type) {
                        // Process the request
                        responder.send(request_processor(request, &mut state, &context)).ok();
                    } else {
                        // No processor available
                        responder.send(None).ok();
                    }
                }

                InternalPropertyRequest::CreatedEntity(entity_id) => { 
                    state.properties.insert(entity_id, HashMap::new());

                    responder.send(None).ok();
                }

                InternalPropertyRequest::DestroyedEntity(entity_id) => {
                    state.properties.remove(&entity_id);

                    responder.send(None).ok();
                }
            }
        }
    })?;

    Ok(())
}

use crate::entity_id::*;
use crate::context::*;
use crate::error::*;
use crate::entity_channel::*;
use crate::stream_entity_response_style::*;
use super::entity_registry::*;
use super::entity_ids::*;

use flo_binding::*;

use futures::prelude::*;
use futures::task;
use futures::task::{Poll, Context};
use futures::future;
use futures::channel::oneshot;

use std::any::{TypeId, Any};
use std::sync::*;
use std::pin::*;
use std::collections::{HashMap};

#[cfg(feature="properties")] 
lazy_static! {
    static ref MESSAGE_PROCESSORS: RwLock<HashMap<TypeId, Box<dyn Send + Sync + Fn(Box<dyn Send + Any>, &mut PropertiesState, &Arc<SceneContext>) -> ()>>> = RwLock::new(HashMap::new());
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

    /// Sends changes to the property to the specified property sink. The stream will be closed if the property is destroyed or if the source stream is destroyed.
    Follow(PropertyReference, PropertySink<TValue>),
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

impl<TValue> PropertySink<TValue> {
    fn send_now(&self, item: TValue) -> Result<(), ()> {
        let waker = {
            let mut core    = self.core.lock().unwrap();

            // Is an error if the stream has closed
            if core.is_closed {
                return Err(());
            }

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
}

impl<TValue> Drop for PropertySink<TValue> {
    fn drop(&mut self) {
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
    }
}

impl<TValue> Sink<TValue> for PropertySink<TValue> {
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Always ready
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: TValue) -> Result<(), Self::Error> {
        self.send_now(item)
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

impl<TValue> Drop for PropertyStream<TValue> {
    fn drop(&mut self) {
        // Mark the core as closed (sink will return an error next time we try to send to it)
        let mut core    = self.core.lock().unwrap();
        core.is_closed  = true;
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
pub async fn properties_channel<TValue>(entity_id: EntityId, context: &Arc<SceneContext>) -> Result<BoxedEntityChannel<'static, PropertyRequest<TValue>, ()>, EntityChannelError>
where
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    // Add a processor for this type if one doesn't already exist
    {
        let mut message_processors = MESSAGE_PROCESSORS.write().unwrap();

        message_processors.entry(TypeId::of::<Option<PropertyRequest<TValue>>>()).or_insert_with(|| {
            Box::new(|message, state, context| process_message::<TValue>(message, state, context))
        });
    }

    // Before returning a channel, wait for the properties entity to become ready
    // (This is most useful at startup when we need the entity tracking to start up before anything else)
    context.send::<_, ()>(PROPERTIES, InternalPropertyRequest::Ready).await.ok();

    // Ensure that the message is converted to an internal request
    context.convert_message::<PropertyRequest<TValue>, InternalPropertyRequest>()?;

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
    /// Used to signal to the property runner that the property is no longer active
    stop_property: Option<oneshot::Sender<()>>,

    /// The current value, if known
    current_value: Option<TValue>,

    /// The sinks where changes to this property should be sent
    sinks: Vec<Option<PropertySink<TValue>>>,
}

impl<TValue> Drop for Property<TValue> {
    fn drop(&mut self) {
        // Signal the future that's running this property that it's done
        if let Some(stop_property) = self.stop_property.take() {
            stop_property.send(()).ok();
        }
    }
}

///
/// The runtime for a single property (dispatches changes to the sinks)
///
async fn run_property<TValue>(property: Arc<Mutex<Property<TValue>>>, values: BindRef<TValue>, stop_receiver: oneshot::Receiver<()>)
where
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    // Loop until the values stream closes, or the stop receiver signals
    future::select(stop_receiver,
        async move {
            let mut values = follow(values).boxed();

            while let Some(value) = values.next().await {
                // Lock the property
                let mut property = property.lock().unwrap();

                // Signal the sinks, freeing any that no longer exist
                for maybe_sink in property.sinks.iter_mut() {
                    if let Some(sink) = maybe_sink {
                        if sink.send_now(value.clone()).is_err() {
                            *maybe_sink = None;
                        }
                    }
                }

                // Update the current value of the property
                property.current_value = Some(value);
            }
        }.boxed()).await;
}

///
/// Processes a message, where the message is expected to be of a particular type
///
fn process_message<TValue>(any_message: Box<dyn Send + Any>, state: &mut PropertiesState, context: &Arc<SceneContext>)
where
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    // Try to unbox the message. The type is Option<PropertyRequest> so we can take it out of the 'Any' reference
    let mut any_message = any_message;
    let message         = any_message.downcast_mut::<Option<PropertyRequest<TValue>>>().and_then(|msg| msg.take());
    let message         = if let Some(message) = message { message } else { return; };

    // The action depends on the message content
    use PropertyRequest::*;
    match message {
        CreateProperty(definition) => { 
            // Create the property
            let (stop_sender, stop_receiver)    = oneshot::channel();
            let property                        = Property::<TValue> {
                stop_property:  Some(stop_sender),
                current_value:  None,
                sinks:          vec![],
            };
            let property                        = Arc::new(Mutex::new(property));

            // Store a copy of the property in the state (we use the entity registry to know which entities exist)
            let owner   = definition.owner;
            let name    = definition.name;
            let values  = definition.values;

            if let Some(entity_properties) = state.properties.get_mut(&owner) {
                entity_properties.insert(name, Box::new(Arc::clone(&property)));
            }

            // Run the property in a background future
            context.run_in_background(run_property(property, values, stop_receiver)).ok();
        }

        DestroyProperty(reference) => {
            if let Some(entity_properties) = state.properties.get_mut(&reference.owner) {
                entity_properties.remove(&reference.name);
            }
        }

        Follow(reference, sink) => {
            // See if there's a property with the appropriate name
            if let Some(property) = state.properties.get_mut(&reference.owner).and_then(|entity| entity.get_mut(&reference.name)) {
                // Try to retrieve the internal value (won't be able to if it's the wrong type)
                if let Some(property) = property.downcast_mut::<Arc<Mutex<Property<TValue>>>>() {
                    let mut property = property.lock().unwrap();

                    // If there's a current value, then send that immediately to the sink
                    if let Some(current_value) = &property.current_value {
                        sink.send_now(current_value.clone()).ok();
                    }

                    // Add the sink to the property
                    if let Some(empty_entry) = property.sinks.iter_mut().filter(|item| item.is_none()).next() {
                        *empty_entry = Some(sink);
                    } else {
                        property.sinks.push(Some(sink));
                    }
                }
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

    // Create the entity itself
    context.create_stream_entity(entity_id, StreamEntityResponseStyle::RespondAfterProcessing, move |context, mut messages| async move {
        // Request updates from the entity registry
        let properties      = context.send_to::<EntityUpdate, ()>(entity_id).unwrap();
        if let Some(mut entity_registry) = context.send_to::<_, ()>(ENTITY_REGISTRY).ok() {
            entity_registry.send(EntityRegistryRequest::TrackEntities(properties)).await.ok();
        }

        while let Some(message) = messages.next().await {
            let message: InternalPropertyRequest = message;

            match message {
                InternalPropertyRequest::Ready => {
                    // This is just used to synchronise requests to the entity
                }

                InternalPropertyRequest::AnyRequest(request) => {
                    // Lock the message processors so we can read from them
                    let message_processors = MESSAGE_PROCESSORS.read().unwrap();

                    // Fetch the ID of the type in the request
                    let request_type = (&*request).type_id();

                    // Try to retrieve a processor for this type (these are created when properties_channel is called to retrieve properties of this type)
                    if let Some(request_processor) = message_processors.get(&request_type) {
                        // Process the request
                        request_processor(request, &mut state, &context);
                    }
                }

                InternalPropertyRequest::CreatedEntity(entity_id) => { 
                    state.properties.insert(entity_id, HashMap::new());
                }

                InternalPropertyRequest::DestroyedEntity(entity_id) => {
                    state.properties.remove(&entity_id);
                }
            }
        }
    })?;

    Ok(())
}

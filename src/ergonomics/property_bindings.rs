use crate::error::*;
use crate::context::*;
use crate::entity_id::*;
use crate::standard_components::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use flo_binding::*;

use std::sync::*;

///
/// Extention functions for handling properties on a scene context
///
pub trait SceneContextPropertiesExt {
    ///
    /// Creates a property from a `flo_binding` binding on the current entity
    ///
    fn property_create<TValue>(&self, property_name: &str, binding: impl Into<BindRef<TValue>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
    where 
        TValue: 'static + PartialEq + Clone + Send + Sized;

    ///
    /// Creates a property from a `flo_binding` binding on a different entity
    ///
    fn property_create_on_entity<TValue>(&self, entity_id: EntityId, property_name: &str, binding: impl Into<BindRef<TValue>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
    where 
        TValue: 'static + PartialEq + Clone + Send + Sized;

    ///
    /// Creates a binding from a known entity ID and property name
    ///
    fn property_bind<TValue>(&self, entity_id: EntityId, property_name: &str) -> BoxFuture<'static, Result<BindRef<TValue>, EntityChannelError>>
    where
        TValue: 'static + Clone + Send + PartialEq;
}

impl SceneContextPropertiesExt for Arc<SceneContext> {
    ///
    /// Creates a property from a `flo_binding` binding on the current entity
    ///
    fn property_create<TValue>(&self, property_name: &str, binding: impl Into<BindRef<TValue>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
    where 
        TValue: 'static + PartialEq + Clone + Send + Sized,
    {
        let context             = Arc::clone(self);
        let entity_id           = self.entity();
        let property_definition = entity_id.map(|entity_id| PropertyDefinition::from_binding(entity_id, property_name, binding));

        async move {
            if let Some(property_definition) = property_definition {
                // Retrieve the channel
                let mut channel = properties_channel::<TValue>(PROPERTIES, &context).await?;

                // Create the property
                channel.send(PropertyRequest::CreateProperty(property_definition)).await?;

                Ok(())
            } else {
                // No entity set
                Err(EntityChannelError::NoCurrentScene)
            }
        }.boxed()
    }

    ///
    /// Creates a property from a `flo_binding` binding on a different entity
    ///
    fn property_create_on_entity<TValue>(&self, entity_id: EntityId, property_name: &str, binding: impl Into<BindRef<TValue>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
    where 
        TValue: 'static + PartialEq + Clone + Send + Sized,
    {
        let context             = Arc::clone(self);
        let property_definition = PropertyDefinition::from_binding(entity_id, property_name, binding);

        async move {
            // Retrieve the channel
            let mut channel = properties_channel::<TValue>(PROPERTIES, &context).await?;

            // Create the property
            channel.send(PropertyRequest::CreateProperty(property_definition)).await?;

            Ok(())
        }.boxed()
    }

    ///
    /// Creates a binding from a known entity ID and property name
    ///
    /// The initial value is used if the property has no value or if the initial value has not been sent yet
    ///
    fn property_bind<TValue>(&self, entity_id: EntityId, property_name: &str) -> BoxFuture<'static, Result<BindRef<TValue>, EntityChannelError>>
    where
        TValue: 'static + Clone + Send + PartialEq,
    {
        let context     = Arc::clone(self);
        let reference   = PropertyReference::new(entity_id, property_name);

        async move {
            // Retrieve the properties channel
            let mut channel     = properties_channel::<TValue>(PROPERTIES, &context).await?;

            // Create a stream and a sink to follow the property
            let (sink, stream)  = property_stream();

            // Request that the channel send values to the sink
            channel.send(PropertyRequest::Follow(reference, sink)).await?;

            // Try to fetch an initial value for the property
            let mut stream      = stream;
            let initial_value   = stream.next().await;
            let initial_value   = if let Some(initial_value) = initial_value {
                initial_value
            } else {
                return Err(EntityChannelError::NoSuchProperty);
            };

            // Create a stream binding from the stream
            let binding = bind_stream(stream, initial_value, |_last_item, item| item);

            // Turn into a BindRef
            Ok(BindRef::from(binding))
        }.boxed()
    }
}

///
/// Creates a property from a `flo_binding` binding on the current entity
///
pub async fn property_create<TValue>(property_name: &str, binding: impl Into<BindRef<TValue>>) -> Result<(), EntityChannelError>
where 
    TValue: 'static + Clone + Send + PartialEq,
{
    SceneContext::current().property_create(property_name, binding).await
}

///
/// Creates a property from a `flo_binding` binding on a different entity
///
pub async fn property_create_on_entity<TValue>(entity_id: EntityId, property_name: &str, binding: impl Into<BindRef<TValue>>) -> Result<(), EntityChannelError>
where 
    TValue: 'static + Clone + Send + PartialEq,
{
    SceneContext::current().property_create_on_entity(entity_id, property_name, binding).await
}

///
/// Creates a binding from a known entity ID and property name
///
pub async fn property_bind<TValue>(entity_id: EntityId, property_name: &str) -> Result<BindRef<TValue>, EntityChannelError>
where
    TValue: 'static + Clone + Send + PartialEq
{
    SceneContext::current().property_bind(entity_id, property_name).await
}

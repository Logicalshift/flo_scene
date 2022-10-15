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

    ///
    /// Creates a rope property from a `RopeBinding` on the current entity
    ///
    fn rope_create<TCell, TAttribute>(&self, property_name: &str, binding: impl Into<RopeBinding<TCell, TAttribute>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
    where 
        TCell:      'static + Send + Unpin + Clone + PartialEq,
        TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default;

    ///
    /// Creates a property from a `flo_binding` binding on a different entity
    ///
    fn rope_create_on_entity<TCell, TAttribute>(&self, entity_id: EntityId, property_name: &str, binding: impl Into<RopeBinding<TCell, TAttribute>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
    where 
        TCell:      'static + Send + Unpin + Clone + PartialEq,
        TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default;

    ///
    /// Creates a binding from a known entity ID and property name
    ///
    fn rope_bind<TCell, TAttribute>(&self, entity_id: EntityId, property_name: &str) -> BoxFuture<'static, Result<RopeBinding<TCell, TAttribute>, EntityChannelError>>
    where 
        TCell:      'static + Send + Unpin + Clone + PartialEq,
        TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default;
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
            // Entity must be started before this binding can complete (TODO: actually only necessary if wait_for_binding fails the first time)
            context.wait_for_entity_to_start(entity_id).await;

            // Retrieve the properties channel
            let mut channel = properties_channel::<TValue>(PROPERTIES, &context).await?;

            // Request that the channel send values to the sink
            let (binding, target) = FloatingBinding::new();
            channel.send(PropertyRequest::Get(reference, target)).await?;

            if let Ok(binding) = binding.wait_for_binding().await {
                Ok(binding)
            } else {
                Err(EntityChannelError::NoSuchProperty)
            }
        }.boxed()
    }

    ///
    /// Creates a rope property from a `RopeBinding` on the current entity
    ///
    fn rope_create<TCell, TAttribute>(&self, property_name: &str, binding: impl Into<RopeBinding<TCell, TAttribute>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
    where 
        TCell:      'static + Send + Unpin + Clone + PartialEq,
        TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default,
    {
        let context             = Arc::clone(self);
        let entity_id           = self.entity();
        let property_definition = entity_id.map(|entity_id| RopePropertyDefinition::from_binding(entity_id, property_name, binding));

        async move {
            if let Some(property_definition) = property_definition {
                // Retrieve the channel
                let mut channel = rope_properties_channel::<TCell, TAttribute>(PROPERTIES, &context).await?;

                // Create the property
                channel.send(RopePropertyRequest::CreateProperty(property_definition)).await?;

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
    fn rope_create_on_entity<TCell, TAttribute>(&self, entity_id: EntityId, property_name: &str, binding: impl Into<RopeBinding<TCell, TAttribute>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
    where 
        TCell:      'static + Send + Unpin + Clone + PartialEq,
        TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default,
    {
        let context             = Arc::clone(self);
        let property_definition = RopePropertyDefinition::from_binding(entity_id, property_name, binding);

        async move {
            // Retrieve the channel
            let mut channel = rope_properties_channel::<TCell, TAttribute>(PROPERTIES, &context).await?;

            // Create the property
            channel.send(RopePropertyRequest::CreateProperty(property_definition)).await?;

            Ok(())
        }.boxed()
    }

    ///
    /// Creates a binding from a known entity ID and property name
    ///
    fn rope_bind<TCell, TAttribute>(&self, entity_id: EntityId, property_name: &str) -> BoxFuture<'static, Result<RopeBinding<TCell, TAttribute>, EntityChannelError>>
    where 
        TCell:      'static + Send + Unpin + Clone + PartialEq,
        TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default,
    {
        let context     = Arc::clone(self);
        let reference   = PropertyReference::new(entity_id, property_name);

        async move {
            // Entity must be started before this binding can complete (TODO: actually only necessary if wait_for_binding fails the first time)
            context.wait_for_entity_to_start(entity_id).await;

            // Retrieve the properties channel
            let mut channel = rope_properties_channel::<TCell, TAttribute>(PROPERTIES, &context).await?;

            // Request that the channel send values to the sink
            let (binding, target) = FloatingBinding::new();
            channel.send(RopePropertyRequest::Get(reference, target)).await?;

            if let Ok(binding) = binding.wait_for_binding().await {
                Ok(binding)
            } else {
                Err(EntityChannelError::NoSuchProperty)
            }
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

///
/// Creates a rope property from a `RopeBinding` on the current entity
///
pub fn rope_create<TCell, TAttribute>(property_name: &str, binding: impl Into<RopeBinding<TCell, TAttribute>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
where 
    TCell:      'static + Send + Unpin + Clone + PartialEq,
    TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default
{
    SceneContext::current().rope_create(property_name, binding)
}

///
/// Creates a property from a `flo_binding` binding on a different entity
///
pub fn rope_create_on_entity<TCell, TAttribute>(entity_id: EntityId, property_name: &str, binding: impl Into<RopeBinding<TCell, TAttribute>>) -> BoxFuture<'static, Result<(), EntityChannelError>>
where 
    TCell:      'static + Send + Unpin + Clone + PartialEq,
    TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default
{
    SceneContext::current().rope_create_on_entity(entity_id, property_name, binding)
}

///
/// Creates a binding from a known entity ID and property name
///
pub fn rope_bind<TCell, TAttribute>(entity_id: EntityId, property_name: &str) -> BoxFuture<'static, Result<RopeBinding<TCell, TAttribute>, EntityChannelError>>
where 
    TCell:      'static + Send + Unpin + Clone + PartialEq,
    TAttribute: 'static + Send + Sync + Unpin + Clone + PartialEq + Default
{
    SceneContext::current().rope_bind(entity_id, property_name)
}

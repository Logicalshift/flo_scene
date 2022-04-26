use crate::entity_channel::*;
use crate::error::*;

use std::any::{TypeId, Any};
use std::collections::{HashMap};

///
/// Stores the data associated with an entity
///
pub struct Entity {
    /// The base entity channels for this entity (which we clone the requested channels from)
    channels: HashMap<TypeId, Box<dyn Any>>,
}

impl Default for Entity {
    fn default() -> Entity {
        Entity {
            channels: HashMap::new()
        }
    }
}

impl Entity {
    ///
    /// Registers a channel as one supported by this entity
    ///
    pub fn register_channel<TMessage, TResponse>(&mut self, channel: EntityChannel<TMessage, TResponse>) -> Result<(), CreateEntityError>
    where
        TMessage:   'static,
        TResponse:  'static,
    {
        let type_id = TypeId::of::<EntityChannel<TMessage, TResponse>>();

        if !self.channels.contains_key(&type_id) {
            // Channel is free
            self.channels.insert(type_id, Box::new(channel));
            Ok(())
        } else {
            // Can only have one channel of a particular type per entity
            Err(CreateEntityError::AlreadyExists)
        }
    }

    ///
    /// If this entity has an implementation of a particular channel, returns it
    ///
    pub fn attach_channel<TMessage, TResponse>(&self) -> Option<EntityChannel<TMessage, TResponse>> 
    where
        TMessage:   'static,
        TResponse:  'static,
    {
        // Attempt to retrieve a channel of this type
        let type_id = TypeId::of::<EntityChannel<TMessage, TResponse>>();
        let channel = self.channels.get(&type_id)?;

        // Downcast the channel back to a concrete type
        let channel = channel.downcast_ref::<EntityChannel<TMessage, TResponse>>()?;

        // Clone it to create the channel for the receiver
        Some(channel.clone())
    }
}

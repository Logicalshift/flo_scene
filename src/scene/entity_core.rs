use crate::entity_channel::*;
use crate::error::*;

use ::desync::scheduler::*;

use std::sync::*;
use std::any::{TypeId, Any};
use std::collections::{HashMap};

///
/// Stores the data associated with an entity
///
pub struct EntityCore {
    /// The base entity channels for this entity (which we clone the requested channels from)
    channels: HashMap<TypeId, Box<dyn Send + Any>>,

    /// The queues for each channel making up this entity
    queues: HashMap<TypeId, Arc<JobQueue>>,
}

impl Default for EntityCore {
    fn default() -> Self {
        EntityCore {
            channels:   HashMap::new(),
            queues:     HashMap::new(),
        }
    }
}

impl EntityCore {
    ///
    /// Registers a channel as one supported by this entity
    ///
    pub fn register_channel<TMessage, TResponse>(&mut self, channel: EntityChannel<TMessage, TResponse>) -> Result<(), CreateEntityError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
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
    /// Creates a queue to run a particular channel
    ///
    pub fn create_queue<TMessage, TResponse>(&mut self, _channel: &EntityChannel<TMessage, TResponse>) -> Result<Arc<JobQueue>, CreateEntityError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        let type_id = TypeId::of::<EntityChannel<TMessage, TResponse>>();
        let queue   = scheduler().create_job_queue();

        if !self.queues.contains_key(&type_id) {
            // Channel is free
            self.queues.insert(type_id, queue.clone());
            Ok(queue)
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
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        // Attempt to retrieve a channel of this type
        let type_id = TypeId::of::<EntityChannel<TMessage, TResponse>>();
        let channel = self.channels.get(&type_id)?;

        // Downcast the channel back to a concrete type
        let channel = channel.downcast_ref::<EntityChannel<TMessage, TResponse>>()?;

        // Clone it to create the channel for the receiver
        Some(channel.clone())
    }

    ///
    /// Removes a channel of a particular type from this entity, and returns true if the entity has other channels
    ///
    pub fn deregister<TMessage, TResponse>(&mut self) -> bool
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        // Attempt to retrieve a channel of this type
        let type_id = TypeId::of::<EntityChannel<TMessage, TResponse>>();

        // Remove from the channels and queues of this entity
        self.channels.remove(&type_id);
        self.queues.remove(&type_id);

        // Result is true if there are still channels
        !self.channels.is_empty()
    }
}

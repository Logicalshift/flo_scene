use crate::entity_channel::*;

use ::desync::scheduler::*;

use std::sync::*;
use std::any::{Any};

///
/// Stores the data associated with an entity
///
pub struct EntityCore {
    /// The channel for sending requests to this entity, stored in an 'Any' box
    channel: Box<dyn Send + Any>,

    /// The queue used for running the entity (this runs the entities main future)
    queue: Arc<JobQueue>,
}

impl EntityCore {
    ///
    /// Creates a new entity that receives messages on the specified channel
    ///
    pub fn new<TMessage, TResponse>(channel: EntityChannel<TMessage, TResponse>) -> EntityCore
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        EntityCore {
            channel:    Box::new(channel),
            queue:      scheduler().create_job_queue(),
        }
    }

    ///
    /// Returns the queue for this entity
    ///
    /// The queue typically just has a single future scheduled on it, so this is usually not useful as nothing
    /// else can run here untl the entity has been finalized
    ///
    pub fn queue(&self) -> Arc<JobQueue> {
        Arc::clone(&self.queue)
    }

    ///
    /// If this entity has an implementation of a particular channel, returns it
    ///
    pub fn attach_channel<TMessage, TResponse>(&self) -> Option<EntityChannel<TMessage, TResponse>> 
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        // Downcast the channel back to a concrete type
        let channel = self.channel.downcast_ref::<EntityChannel<TMessage, TResponse>>()?;

        // Clone it to create the channel for the receiver
        Some(channel.clone())
    }
}

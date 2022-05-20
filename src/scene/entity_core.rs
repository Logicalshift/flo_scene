use super::background_future::*;
use crate::any_entity_channel::*;
use crate::simple_entity_channel::*;

use ::desync::scheduler::*;
use futures::prelude::*;

use std::sync::*;
use std::any::{Any, TypeId, type_name};

///
/// Stores the data associated with an entity
///
pub (crate) struct EntityCore {
    /// A background future core: can be used to schedule other tasks to be performed by this entity
    background_futures: Weak<Mutex<BackgroundFutureCore>>,

    /// A conversion channel, which has the same response type but the message type is `Box<dyn Any + Send>`. This is of type `BoxedMessageChannel<TResponse>`
    create_conversion_channel: Box<dyn Send + Fn() -> AnyEntityChannel>,

    /// The channel for sending requests to this entity, stored in an 'Any' box. This is of type `SimpleEntityChannel<TMessage, TResponse>`
    channel: Box<dyn Send + Any>,

    /// The queue used for running the entity (this runs the entities main future)
    queue: Arc<JobQueue>,

    /// The type ID of the message processed 'natively' by this entity
    message_type_id: TypeId,

    /// The name of the message type for this entity
    message_type_name: &'static str,

    /// The type ID of the response processed 'natively' by this entity
    response_type_id: TypeId,

    /// The name of the response type for this entity
    response_type_name: &'static str,
}

impl EntityCore {
    ///
    /// Creates a new entity that receives messages on the specified channel
    ///
    pub fn new<TMessage, TResponse>(channel: SimpleEntityChannel<TMessage, TResponse>, background_future: &BackgroundFuture) -> EntityCore
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        let conversion_channel          = channel.clone();
        let create_conversion_channel   = move || { AnyEntityChannel::from_channel(conversion_channel.clone()) };

        EntityCore {
            background_futures:         Arc::downgrade(&background_future.core()),
            create_conversion_channel:  Box::new(create_conversion_channel),
            channel:                    Box::new(channel),
            queue:                      scheduler().create_job_queue(),
            message_type_id:            TypeId::of::<TMessage>(),
            response_type_id:           TypeId::of::<TResponse>(),
            message_type_name:          type_name::<TMessage>(),
            response_type_name:         type_name::<TResponse>(),
        }
    }

    ///
    /// Retrieves the message processed 'natively' by this channel
    ///
    pub fn message_type_id(&self) -> TypeId {
        self.message_type_id
    }

    ///
    /// Retrieves the response processed 'natively' by this channel
    ///
    pub fn response_type_id(&self) -> TypeId {
        self.response_type_id
    }

    ///
    /// Retrieves the message processed 'natively' by this channel
    ///
    pub fn message_type_name(&self) -> String {
        self.message_type_name.to_string()
    }

    ///
    /// Retrieves the response processed 'natively' by this channel
    ///
    pub fn response_type_name(&self) -> String {
        self.response_type_name.to_string()
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
    pub fn attach_channel<TMessage, TResponse>(&self) -> Option<SimpleEntityChannel<TMessage, TResponse>> 
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
    {
        // Downcast the channel back to a concrete type
        let channel = self.channel.downcast_ref::<SimpleEntityChannel<TMessage, TResponse>>()?;

        // Clone it to create the channel for the receiver
        Some(channel.clone())
    }

    ///
    /// Returns the channel with polymorphic messages and responses. The messages here unwrap to `Option<TMessage>` and `Option<TResponse>`
    ///
    pub fn attach_channel_any(&self) -> AnyEntityChannel {
        (self.create_conversion_channel)()
    }

    ///
    /// Stops the tasks associated with this entity from running
    ///
    pub fn stop(&self) {
        if let Some(background_futures) = self.background_futures.upgrade() {
            background_futures.stop();
        }
    }

    ///
    /// Adds a future to run in the background of this entity
    ///
    /// This future will be dropped if this entity is destroyed (eg, by the main loop ending)
    ///
    pub fn run_in_background(&self, future: impl 'static + Send + Future<Output=()>) {
        if let Some(background_futures) = self.background_futures.upgrade() {
            background_futures.add_future(future);
        }
    }
}

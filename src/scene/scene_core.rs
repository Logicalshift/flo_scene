use super::entity_core::*;
use super::map_entity_type::*;

use crate::error::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::entity_channel_ext::*;
use crate::simple_entity_channel::*;
use crate::message::*;
use crate::context::*;

use ::desync::scheduler::*;

use futures::prelude::*;
use futures::channel::oneshot;
use futures::stream::{BoxStream};
use futures::future;
use futures::future::{BoxFuture};

use std::any::{TypeId, Any};
use std::sync::*;
use std::collections::{HashMap};

///
/// The scene core represents the state shared between all entities in a scene
///
pub struct SceneCore {
    /// The entities that are available in this core
    pub (super) entities: HashMap<EntityId, Arc<Mutex<EntityCore>>>,

    /// Futures waiting to run the entities in this scene
    pub (super) waiting_futures: Vec<BoxFuture<'static, ()>>,

    /// Used by the scene that owns this core to request wake-ups (only one scene can be waiting for a wake up at once)
    pub (super) wake_scene: Option<oneshot::Sender<()>>,

    /// Provides a function for mapping from one entity channel type to another, based on the message type
    map_for_message: HashMap<TypeId, HashMap<TypeId, MapEntityType>>,
}

impl Default for SceneCore {
    fn default() -> SceneCore {
        SceneCore {
            entities:           HashMap::new(),
            waiting_futures:    vec![],
            wake_scene:         None,
            map_for_message:    HashMap::new(),
        }
    }
}

impl SceneCore {
    ///
    /// Creates an entity that processes a particular kind of message
    ///
    pub (crate) fn create_entity<TMessage, TResponse, TFn, TFnFuture>(&mut self, scene_context: Arc<SceneContext>, runtime: TFn) -> Result<(), CreateEntityError>
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send,
        TFn:        'static + Send + FnOnce(BoxStream<'static, Message<TMessage, TResponse>>) -> TFnFuture,
        TFnFuture:  'static + Send + Future<Output = ()>,
    {
        // The entity ID is specified in the supplied scene context
        let entity_id           = scene_context.entity().unwrap();

        // The entity must not already exist
        if self.entities.contains_key(&entity_id) { return Err(CreateEntityError::AlreadyExists); }

        // Create the channel and the eneity
        let (channel, receiver) = SimpleEntityChannel::new(5);
        let entity              = Arc::new(Mutex::new(EntityCore::new(channel)));
        let queue               = entity.lock().unwrap().queue();

        self.entities.insert(entity_id, entity);

        // Start the future running
        let future              = async move {
            let future = scheduler().future_desync(&queue, move || async move {
                // Start the future running
                let receiver            = receiver.boxed();
                let mut runtime_future  = SceneContext::with_context(&scene_context, || runtime(receiver).boxed()).unwrap();

                // Poll it in the scene context
                future::poll_fn(|ctxt| {
                    SceneContext::with_context(&scene_context, || 
                        runtime_future.poll_unpin(ctxt)).unwrap()
                }).await;

                // Return the context once we're done
                scene_context
            }.boxed());

            // Run the future, and retrieve the scene context
            let scene_context = future.await.ok();

            // When done, deregister the entity
            if let Some(scene_context) = scene_context {
                scene_context.finish_entity::<TMessage, TResponse>(entity_id);
            }
        };
        let future              = future.boxed();

        // Queue a request in the runtime that we will run the entity
        self.waiting_futures.push(future);

        // Wake up the scene so it can schedule this future
        if let Some(wake_scene) = self.wake_scene.take() {
            wake_scene.send(()).ok();
        }

        Ok(())
    }

    ///
    /// Specifies that if an entity accepts messages in the format `TOriginalMessage` that these can be converted to `TNewMessage`
    ///
    pub (crate) fn convert_message<TOriginalMessage, TNewMessage>(&mut self)
    where
        TOriginalMessage:   'static + Send,
        TNewMessage:        'static + Send + From<TOriginalMessage>,
    {
        // Create a converter from TOriginalMessage to TNewMessage
        let converter       = MapEntityType::new::<TOriginalMessage, TNewMessage>();
        let original_type   = TypeId::of::<TOriginalMessage>();
        let new_type        = TypeId::of::<TNewMessage>();

        // Any entity that accepts TNewMessage can also accept TOriginalMessage
        self.map_for_message.entry(new_type).or_insert_with(|| HashMap::new())
            .insert(original_type, converter);
    }

    ///
    /// Requests that we send messages to a channel for a particular entity
    ///
    pub (crate) fn send_to<TMessage, TResponse>(&mut self, entity_id: EntityId) -> Result<BoxedEntityChannel<'static, TMessage, TResponse>, EntityChannelError> 
    where
        TMessage:   'static + Send,
        TResponse:  'static + Send, 
    {
        // Try to retrieve the entity
        let entity = self.entities.get(&entity_id);
        let entity = if let Some(entity) = entity { entity } else { return Err(EntityChannelError::NoSuchEntity); };
        
        // Attach to the channel in the entity that belongs to this stream type
        let channel = entity.lock().unwrap().attach_channel();
        
        if let Some(channel) = channel { 
            // Return the direct channel
            Ok(channel.boxed()) 
        } else {
            // Attempt to convert the message if possible
            let target_message      = entity.lock().unwrap().message_type_id();
            let source_message      = TypeId::of::<TMessage>();
            let message_converter   = self.map_for_message.get(&target_message).and_then(|target_hash| target_hash.get(&source_message));

            if let Some(message_converter) = message_converter {
                // Hrm, the problem here is we know the target message type but not the source type
                // We can know both in the mapper but that doesn't know enough about the response type
                todo!()
            } else {
                Err(EntityChannelError::NotListening)
            }
        }
    }

    ///
    /// Called when an entity in this context has finished
    ///
    pub (crate) fn finish_entity(&mut self, entity_id: EntityId) {
        self.entities.remove(&entity_id);
    }
}

///
/// Creates a channel that accepts messages that implement `Box<dyn Send + Any>` and unpack as 'Option<Self::Message>'
///
fn channel_from_any_message<TChannel>(channel: TChannel) -> impl EntityChannel<Message=Box<dyn Send + Any>, Response=TChannel::Response>
where
    TChannel:           EntityChannel,
    TChannel::Message:  'static,
    TChannel::Response: 'static,
{
    channel.map(|boxed_message: Box<dyn Send + Any>| {
        // Unbox the message, assuming it's the right type
        let mut boxed_message   = boxed_message;
        let unboxed             = boxed_message.downcast_mut::<Option<TChannel::Message>>();
        let unboxed             = unboxed.expect("Boxed message must be of the expected type");
        let unboxed             = unboxed.take().expect("Can only unbox message once");

        unboxed
    }, |response| response)
}

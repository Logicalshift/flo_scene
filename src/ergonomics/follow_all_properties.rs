use super::property_bindings::*;
use super::entity_channel_ext::*;

use crate::error::*;
use crate::context::*;
use crate::entity_id::*;
use crate::entity_channel::*;
use crate::standard_components::*;
use crate::simple_entity_channel::*;

use futures::stream;
use futures::prelude::*;
use futures::channel::mpsc;
use futures::channel::oneshot;

use flo_stream::*;
use flo_binding::*;

use std::sync::*;
use std::collections::{HashMap};

///
/// Represents a change to a value of a property on an entity
///
#[derive(Clone, PartialEq, Debug)]
pub enum FollowAll<TValue> {
    /// Property was set to a new value on a particular entity
    NewValue(EntityId, TValue),

    /// A property value was removed from an entity (or the entity itself was destroyed)
    Destroyed(EntityId),

    /// An error occurred
    Error(EntityChannelError),
}

enum FollowAllEvent<TValue> {
    PropertyUpdate(PropertyUpdate),
    NewValue(EntityId, TValue)
}

///
/// Follows the values set for all properties of a particular name and type across an entire scene
///
pub fn properties_follow_all<TValue>(context: &Arc<SceneContext>, property_name: &str) -> impl Stream<Item=FollowAll<TValue>> 
where
    TValue: 'static + PartialEq + Clone + Send + Sized,
{
    // Set up the state
    let property_name   = property_name.to_string();
    let context         = Arc::clone(context);
    let current_entity  = context.entity().unwrap_or(PROPERTIES);

    // Result is a generator stream
    generator_stream(move |yield_value| async move {
        // Connect to the properties channel, and strip out any errors
        let channel     = properties_channel::<TValue>(PROPERTIES, &context).await;
        let mut channel = match channel {
            Ok(channel)     => channel,
            Err(err)        => { 
                yield_value(FollowAll::Error(err));
                return;
            }
        };

        // Track the properties across all entities
        let (sender, receiver) = SimpleEntityChannel::new(current_entity, 1);
        let send_ok = channel.send(PropertyRequest::TrackPropertiesWithName(property_name.clone(), sender.boxed())).await;

        // Create a stream of streams, and flatten it. We can send new streams to follow_streams and they'll get processed in our main loop
        let (follow_streams, follow_values) = mpsc::channel(10);
        let follow_values                   = follow_values.flatten_unordered(None);

        // Stop on error
        if let Err(err) = send_ok { yield_value(FollowAll::Error(err)); return; };

        // Now we can track when properties are actually created and destroyed, and follow their values too
        let follow_values       = follow_values.map(|(owner, value)| FollowAllEvent::NewValue(owner, value));
        let receiver            = receiver.map(|update| FollowAllEvent::PropertyUpdate(update));
        let mut receiver        = stream::select(follow_values, receiver);
        let mut follow_streams  = follow_streams;
        let mut when_destroyed  = HashMap::new();

        while let Some(update) = receiver.next().await {
            use PropertyUpdate::*;

            match update {
                FollowAllEvent::PropertyUpdate(Created(PropertyReference { owner, .. }))    => {
                    // Attempt to fetch the property
                    if let Ok(property) = context.property_bind::<TValue>(owner, &property_name).await {
                        let (signal_finished, on_finished)  = oneshot::channel::<()>();
                        let value_stream                    = follow(property).map(move |value| (owner, value));
                        let value_stream                    = value_stream.take_until(on_finished);

                        // Send the value stream to follow_streams so that it generates events
                        follow_streams.send(value_stream).await.ok();

                        // Stop the events for this property when it is destroyed
                        when_destroyed.insert(owner, signal_finished);
                    }
                }

                FollowAllEvent::PropertyUpdate(Destroyed(PropertyReference { owner, .. }))  => {
                    // Finish the value stream
                    if let Some(on_finished) = when_destroyed.remove(&owner) {
                        on_finished.send(()).ok();

                        // Signal that this property was destroyed
                        yield_value(FollowAll::Destroyed(owner));
                    }
                }

                FollowAllEvent::NewValue(owner, value) => {
                    // In case we get extra events, the 'when_destroyed' value must not be signalled for this owner
                    if when_destroyed.contains_key(&owner) {
                        yield_value(FollowAll::NewValue(owner, value));
                    }
                }
            }
        }
    })
}

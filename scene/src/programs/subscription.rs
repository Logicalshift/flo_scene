use crate::error::*;
use crate::output_sink::*;
use crate::scene_context::*;
use crate::scene_message::*;
use crate::stream_target::*;

use futures::prelude::*;

use std::marker::{PhantomData};

use serde::*;

///
/// Sub-programs that can send events should support this 'Subscribe' message (via a filter). This is a request that the
/// program should send its events to the sender of the message: this is useful for messages that work like events that
/// can be sent to multiple targets.
///
/// The type parameter is used to specify what type of message the subscription will return. This can be used to support
/// multiple subscriptions of different types, but this should generally be avoided if possible: it's more useful as
/// a way of ensuring that the expected events are subscribed to and to subscribe to events without knowing the source
/// (via, the `connect()` function in `Scene`)
///
/// It's better to use an output stream so that `connect()` can be most easily used to specify where the events are going.
///
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Subscribe<TMessageType: SceneMessage>(StreamTarget, PhantomData<TMessageType>);

impl<TMessageType: SceneMessage> SceneMessage for Subscribe<TMessageType> { }

impl<TMessageType: SceneMessage> Subscribe<TMessageType> { 
    ///
    /// Creates a 'subscribe' message that will send its requests to the specified target
    ///
    #[inline]
    pub fn with_target(target: StreamTarget) -> Self {
        Subscribe(target, PhantomData)
    }

    ///
    /// Retrieves the place where the messages for this subscription should be sent
    ///
    #[inline]
    pub fn target(&self) -> StreamTarget {
        self.0.clone()
    }
}

///
/// Creates a 'Subscribe' message that will return a particular type
///
#[inline]
pub fn subscribe<TMessageType: SceneMessage>(target: impl Into<StreamTarget>) -> Subscribe<TMessageType> {
    Subscribe::with_target(target.into())
}

///
/// Stores the subscribers for an event stream, and forwards events as needed
///
pub struct EventSubscribers<TEventMessage>
where
    TEventMessage: 'static + SceneMessage,
{
    /// The output sinks that will receive the events from this subprogram
    receivers: Vec<OutputSink<TEventMessage>>,

    /// The next receiver to use when sending a round-robin message
    next_receiver: usize,
}

impl<TEventMessage> EventSubscribers<TEventMessage>
where
    TEventMessage: 'static + SceneMessage,
{
    ///
    /// Creates a new set of event subscribers
    ///
    pub fn new() -> Self {
        EventSubscribers { 
            receivers:      vec![],
            next_receiver:  0,
        }
    }

    ///
    /// Subscribes a subprogram to the events sent by this object
    ///
    pub fn subscribe(&mut self, context: &SceneContext, target: impl Into<StreamTarget>) {
        let target = target.into();

        // Remove any subscriber that's no longer attached to a target
        self.receivers.retain(|sink| sink.is_attached());

        // If we can successfully connect to the target, then send events there
        let output_sink = context.send(target);
        let output_sink = if let Ok(output_sink) = output_sink { output_sink } else { return; };

        self.receivers.push(output_sink);
    }

    ///
    /// Adds a target output sink to the list of subscribers for this object
    ///
    /// This sink cannot be unsubscribed from the events, but this can be used to send to other streams where the target is not identified by a subprogram ID
    ///
    pub fn add_target(&mut self, output_sink: OutputSink<TEventMessage>) {
        self.receivers.push(output_sink)
    }

    ///
    /// Sends a message to a single subscriber, returning Ok(()) if the message is delivered, otherwise returning an error that preserves the original message
    ///
    /// Subscribers are sent to in a round-robin fashion
    ///
    pub async fn send_round_robin(&mut self, message: TEventMessage) -> Result<(), TEventMessage> {
        let mut message = message;

        loop {
            // If there are no receivers, then there's onothing to send a message to
            if self.receivers.is_empty() {
                break Err(message);
            }

            // Move on to the next receiver
            self.next_receiver += 1;
            if self.next_receiver >= self.receivers.len() {
                self.next_receiver = 0;
            }

            // Try to send to this receiver
            match self.receivers[self.next_receiver].send(message).await {
                Ok(()) => { break Ok(()); }

                Err(SceneSendError::TargetProgramEndedBeforeReady)  |
                Err(SceneSendError::ErrorAfterDeserialization)      |
                Err(SceneSendError::CannotReEnterTargetProgram)     => {
                    // The message was sent but was not processed by the target (we treat this as 'Ok' because we can't get it back)
                    break Ok(());
                }

                Err(SceneSendError::StreamClosed(returned_message))                             |
                Err(SceneSendError::CannotAcceptMoreInputUntilSceneIsIdle(returned_message))    |
                Err(SceneSendError::TargetProgramEnded(returned_message))                       |
                Err(SceneSendError::CannotDeserialize(returned_message))                        |
                Err(SceneSendError::StreamDisconnected(returned_message))                       => {
                    // Remove this subscriber as it errored out
                    self.receivers.remove(self.next_receiver);

                    // Retry the same index again next time through the loop
                    if self.next_receiver > 0 {
                        self.next_receiver -= 1;
                    } else if !self.receivers.is_empty() {
                        self.next_receiver = self.receivers.len() - 1;
                    }

                    // Retry the message on the next subscriber
                    message = returned_message;
                }
            }
        }
    }
}


impl<TEventMessage> EventSubscribers<TEventMessage>
where
    TEventMessage: 'static + Clone + SceneMessage,
{
    ///
    /// Sends a message to the subscribers to this object
    ///
    /// Returns true if the message is sent to at least one subscriber, or false if there are no subscribers
    ///
    pub async fn send(&mut self, message: TEventMessage) -> bool {
        // Remove any subscriber that's no longer attached to a target
        self.receivers.retain(|sink| sink.is_attached());

        // Send to all of the streams at once
        let senders = self.receivers.iter_mut()
            .enumerate()
            .map(|(idx, sender)| sender.send(message.clone()).map(move |result| (idx, result)))
            .collect::<Vec<_>>();

        // Wait for all the messages to send
        let mut results = future::join_all(senders).await;

        // Remove any subscribers that generated an error from the subscribers (iterating through the indexes in reverse so we can remove )
        let mut sent_successfully = false;

        results.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (idx, result) in results.into_iter().rev() {
            if result.is_err() {
                // Remove any susbcriber that's no longer attached
                self.receivers.remove(idx);
            } else {
                // At least one message was delivered
                sent_successfully = true;
            }
        }

        // Result is true if we sent at least one event, or false otherwise
        sent_successfully
    }
}

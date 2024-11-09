use super::poll_action::*;
use super::poll_result::*;
use super::stream_id::*;
use crate::host::*;

use futures::prelude::*;
use futures::task::{Poll, Waker};
use futures::channel::mpsc;

use std::collections::{HashMap};
use std::sync::*;

///
/// Runs a guest subprogram as a subprogram in a scene
///
/// The result stream here should supply messages only for the subprogram that should be run here.
///
/// The guest program should generate the supplied message type, it's an error if it does not.
///
pub async fn run_host_subprogram<TMessageType>(input_stream: InputStream<TMessageType>, context: SceneContext, actions: mpsc::Sender<GuestAction>, results: impl 'static + Send + Unpin + Stream<Item=GuestResult>) 
where
    TMessageType: 'static + SceneMessage
{
    let mut results = results;

    let guest_program_handle;
    let guest_stream_id;

    // Setup phase: we get the program handle and the input stream handle for the guest program
    loop {
        if let Some(msg) = results.next().await {
            match msg {
                GuestResult::CreateSubprogram(program_id, program_handle, stream_id) => {
                    // TODO: program_id does not need to match here but maybe we should check/warn if it does not
                    if Some(program_id) != context.current_program_id() {
                        // Program IDs do not match: log warning (consider error)
                    }

                    guest_program_handle    = program_handle;
                    guest_stream_id         = stream_id;
                    break;
                }

                unexpected => {
                    // Unexpected message
                    // TODO: log/soft error instead of panicking
                    panic!("Unexpected guest message: {:?}", unexpected);
                }
            }
        } else {
            // Guest program failed to start
            // TODO: log/soft error instead of panicking
            panic!("Guest program failed to start");
        }
    }

    // Guest program has started: perform 'pre-flight' checks
    if guest_stream_id != HostStreamId::for_message::<TMessageType>() {
        // The guest program must generate the same stream ID as the host
        // TODO: log/soft error instead of panicking
        panic!("Was expecting a guest program generating message type {:?}, but got {:?}", HostStreamId::for_message::<TMessageType>(), guest_stream_id);
    }

    // Signal used to indicate when we can send a message we've received that's destined for this program. This is basically just a semaphore we can poll for
    let signal_ready        = Arc::new(Mutex::new((None, false)));
    let wait_ready          = signal_ready.clone();
    let message_actions     = actions.clone();
    let control_actions     = actions;

    // Main loop: relay messages and connect to sinks
    future::select(
        Box::pin(async move {
            use GuestResult::*;

            let mut control_actions = control_actions;
            let mut active_sinks    = HashMap::new();

            // Loop 1: handle the results from the guest program
            while let Some(result) = results.next().await {
                match result {
                    Stopped => { 
                        // Guest has entirely stopped
                        break;
                    }

                    CreateSubprogram(_id, _handle, _stream_id) => {
                        // TODO: we don't support subprograms other than our own
                    }

                    EndedSubprogram(program_handle) => {
                        // Program that we're running has entirely stopped 
                        if program_handle == guest_program_handle {
                            break;
                        }
                    }

                    Ready(handle) => {
                        if handle == guest_program_handle {
                            // Indicate we're ready to send more input
                            let waker = {
                                let (waker, is_ready)           = &mut *signal_ready.lock().unwrap();
                                let waker: &mut Option<Waker>   = waker;

                                *is_ready = true;
                                waker.take()
                            };

                            // Wake up anything that's waiting for the input stream to become ready
                            if let Some(waker) = waker {
                                waker.wake();
                            }
                        }
                    }

                    Connect(sink_handle, stream_target) => {
                        // Get the host streams that we want to connect to
                        let stream_id   = stream_target.stream_id();

                        if let Some(stream_id) = stream_id  {
                            let target      = stream_target.to_stream_target();

                            // Ask the encoder to do the attachment
                            match connect(stream_id, target, &context) {
                                Ok(sink) => {
                                    // Store this sink
                                    active_sinks.insert(sink_handle, sink);

                                    // Indicate that we're ready
                                    if control_actions.send(GuestAction::Ready(sink_handle)).await.is_err() { return; }
                                }

                                Err(err) => {
                                    // Could not connect this sink
                                    if control_actions.send(GuestAction::SinkConnectionError(sink_handle, err)).await.is_err() { return; }
                                }
                            }
                        } else {
                            // We can't deserialize this stream within this scene
                            // TODO: if there's no way to deserialize this sink we can potentially still send it between guest programs (we need a way to distinguish stream IDs that use the same type to do this)
                            if control_actions.send(GuestAction::SinkConnectionError(sink_handle, ConnectionError::StreamNotKnown)).await.is_err() { return; }
                        }
                    }

                    Send(sink_handle, encoded_bytes) => {
                        // Send to an existing connected sink handle
                        // TODO: perform the send in parallel with the other waiting messages
                        // We don't usually need to do this if there's only one program in the guest as the guest will usually just be waiting for the ready, but for
                        // multiple programs or guest programs that use something like 'select' this will improve performance
                        if let Some(sink) = active_sinks.get_mut(&sink_handle) {
                            match sink.send(encoded_bytes).await {
                                Ok(()) => {
                                    // Message was sent, sink is ready again
                                    if control_actions.send(GuestAction::Ready(sink_handle)).await.is_err() { return; }
                                }

                                Err(err) => {
                                    // Report the error to the guest program
                                    if control_actions.send(GuestAction::SinkError(sink_handle, err)).await.is_err() { return; }
                                    if control_actions.send(GuestAction::Ready(sink_handle)).await.is_err() { return; }
                                }
                            }
                        } else {
                            // Sink is not connected
                            if control_actions.send(GuestAction::SinkError(sink_handle, SceneSendError::StreamDisconnected(encoded_bytes))).await.is_err() { return; }
                        }
                    }

                    Disconnect(sink_handle) => {
                        // Remove a sink handle (which should disconnect it)
                        active_sinks.remove(&sink_handle);
                    }

                    ContinuePolling => { 
                        // Nothing for us to do, should be handled by the stream
                    }
                }
            } 
        }),

        Box::pin(async move {
            let mut message_actions = message_actions;

            // Loop 2: read from the input stream
            let mut input_stream = input_stream;
            while let Some(input) = input_stream.next().await {
                // Wait for the input stream to become ready (and mark it as 'not ready' in anticipation of the message we're sending)
                let wait_ready = wait_ready.clone();
                future::poll_fn(|context| {
                    let (waker, is_ready) = &mut *wait_ready.lock().unwrap();

                    if *is_ready {
                        *is_ready = false;
                        Poll::Ready(())
                    } else {
                        *waker = Some(context.waker().clone());
                        Poll::Pending
                    }
                }).await;

                // Encode the input stream and send it
                // TODO: we probably want some better error handling here if we can't encode a message (do we ignore it? stop the program?)
                let encoded_input = input.to_guest_message().map_err(|_| ()).unwrap();

                if message_actions.send(GuestAction::SendMessage(guest_program_handle, encoded_input)).await.is_err() {
                    // Just stop if there's any error sending to the guest program
                    break;
                }
            }
        })
    ).await;
}

///
/// Creates a connection that sends to a host stream by decoding messages from a guest
///
fn connect(stream_id: StreamId, target: StreamTarget, context: &SceneContext) -> Result<impl Send + Unpin + Sink<Vec<u8>, Error=SceneSendError<Vec<u8>>>, ConnectionError> {
    // Create the stream target
    let serialized_target = SerializedStreamTarget::from(stream_id);
    let serialized_target = match target {
        StreamTarget::None | StreamTarget::Any  => Ok(serialized_target),
        StreamTarget::Program(program_id)       => todo!("Cannot map a target program to a specific stream ID at the moment"),
        StreamTarget::Filtered(_, _)            => Err(ConnectionError::FilterMappingMissing)
    }?;

    // Send as a postcard stream
    let postcard_stream = context.send_serialized::<GuestMessage>(serialized_target)?;

    // Put a postcard deserialzer in front of the stream
    let postcard_stream = postcard_stream
        .sink_map_err(|err| err.map(|msg| postcard::to_stdvec(&msg).unwrap_or_else(|_| vec![])))
        .with(|bytes: Vec<u8>| async move {
            Ok(GuestMessage(bytes))
        });

    Ok(Box::pin(postcard_stream))
}

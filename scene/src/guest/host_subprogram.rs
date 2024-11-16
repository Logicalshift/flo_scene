use super::poll_action::*;
use super::poll_result::*;
use super::stream_id::*;
use crate::host::*;
use crate::util::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::stream::{BoxStream};
use futures::task::{Poll, Waker};

use std::collections::{VecDeque, HashMap, HashSet};
use std::sync::*;

///
/// Data associated with a stream originating from the guest side of the connection
///
struct GuestStreamCore {
    /// Messages that have been sent from the guest (there should only be one message here as we need to signal 'ready' to receive more, but we will cache anything we get)
    pending: VecDeque<Vec<u8>>,

    /// Flag that's set once the guest indicates it has closed the stream
    closed: bool,

    /// Waker for the stream
    waker: Option<Waker>,
}

///
/// Structure used to manage the streams in a host subprogram (guest/host)
///
struct HostStreams {
    /// Streams that are ready
    ready_streams: HashSet<SerializationId>,

    // Streams that are closed
    closed_streams: HashSet<SerializationId>,

    /// Wakers for when a stream becomes ready or closed
    stream_waker: HashMap<SerializationId, Option<Waker>>,

    /// Targets for the streams from the guest
    guest_streams: HashMap<SerializationId, Arc<Mutex<GuestStreamCore>>>,

    /// The next ID to assign to a stream
    next_stream_id: usize,
}

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
    let signal_ready                = Arc::new(Mutex::new((None, false)));
    let wait_ready                  = signal_ready.clone();
    let message_actions             = actions.clone();
    let control_actions             = actions;
    let streams                     = Arc::new(Mutex::new(HostStreams { ready_streams: HashSet::new(), closed_streams: HashSet::new(), stream_waker: HashMap::new(), guest_streams: HashMap::new(), next_stream_id: 0 }));
    let (future_pile, pile_runner)  = FuturePile::new();
    let message_streams             = streams.clone();
    let message_future_pile         = future_pile.clone();

    // Main loop: relay messages and connect to sinks
    future::select_all(vec![
        async move {
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
                            let target                  = stream_target.to_stream_target();
                            let serialization_context   = HostSerializationContext(streams.clone(), control_actions.clone(), future_pile.clone());

                            // Ask the encoder to do the attachment
                            match connect(stream_id, target, &context, serialization_context) {
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

                    SendStream(stream_id, msg) => {
                        // Fetch the core for the stream that is being sent to
                        let stream_core = {
                            let streams = streams.lock().unwrap();
                            streams.guest_streams.get(&stream_id).cloned()
                        };

                        if let Some(stream_core) = stream_core {
                            // Stream exists: send the message and retrieve the waker
                            let waker = {
                                let mut stream_core = stream_core.lock().unwrap();
                                stream_core.pending.push_back(msg);
                                stream_core.waker.take()
                            };

                            // Wake the stream
                            if let Some(waker) = waker { waker.wake() };
                        }
                    }

                    ReadyStream(stream_id) => {
                        let waker = {
                            // Mark the stream as ready, then wake anything up that's waiting on it
                            let mut streams = streams.lock().unwrap();

                            streams.ready_streams.insert(stream_id);
                            streams.stream_waker.get_mut(&stream_id)
                                .map(|waker| waker.take())
                                .unwrap_or(None)
                        };

                        if let Some(waker) = waker {
                            waker.wake();
                        }
                    }

                    CloseStream(stream_id) => {
                        let waker = {
                            // Mark the stream as closed, then wake anything up that's waiting on it
                            let mut streams = streams.lock().unwrap();

                            if let Some(stream_core) = streams.guest_streams.get(&stream_id).cloned() {
                                // This is a stream that's receiving from the guest: mark it as closed and drop the core
                                use std::mem;

                                streams.guest_streams.remove(&stream_id);
                                mem::drop(streams);

                                let mut stream_core = stream_core.lock().unwrap();
                                stream_core.closed = true;

                                stream_core.waker.take()
                            } else {
                                // This is a stream sending to the guest: mark it as closed so it won't try to send any more messages
                                streams.closed_streams.insert(stream_id);
                                streams.stream_waker.get_mut(&stream_id)
                                    .map(|waker| waker.take())
                                    .unwrap_or(None)
                            }
                        };

                        if let Some(waker) = waker {
                            waker.wake();
                        }
                    }
                }
            } 
        }.boxed(),

        async move {
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
                let encoded_input = input.to_guest_message(&HostSerializationContext(message_streams.clone(), message_actions.clone(), message_future_pile.clone())).map_err(|_| ()).unwrap();

                if message_actions.send(GuestAction::SendMessage(guest_program_handle, encoded_input)).await.is_err() {
                    // Just stop if there's any error sending to the guest program
                    break;
                }
            }
        }.boxed(),

        pile_runner.run_forever().boxed()
        ]
    ).await;
}

///
/// Creates a connection that sends to a host stream by decoding messages from a guest
///
fn connect(stream_id: StreamId, target: StreamTarget, context: &SceneContext, serialization_context: HostSerializationContext) -> Result<impl Send + Unpin + Sink<Vec<u8>, Error=SceneSendError<Vec<u8>>>, ConnectionError> {
    let raw_stream = stream_id.send_guest_messages(target, context, serialization_context)?;

    Ok(Box::into_pin(raw_stream))
}

impl HostStreams {
    /// Retrieves a unique stream ID for a new stream
    pub fn next_stream_id(&mut self) -> SerializationId {
        let id = self.next_stream_id;
        self.next_stream_id += 1;

        SerializationId::SimpleStream(id)
    }
}

///
/// The host serialization context can be used to create streams to exchange data with the guest side
///
#[derive(Clone)]
struct HostSerializationContext(Arc<Mutex<HostStreams>>, mpsc::Sender<GuestAction>, FuturePile);

impl SerializationContext for HostSerializationContext {
    fn send_stream(&self, stream: BoxStream<'static, Vec<u8>>) -> Result<SerializationId, SceneSendError<BoxStream<'static, Vec<u8>>>> {
        // Create a stream ID, and create a copy of the host streams and action sender
        let host_streams    = self.0.clone();
        let guest_actions   = self.1.clone();
        let new_stream_id   = host_streams.lock().unwrap().next_stream_id();

        // State from polling the stream
        enum State {
            Closed,
            Ready
        }

        // Create a future that runs the stream
        let run_stream = async move {
            let mut stream          = stream;
            let mut guest_actions   = guest_actions;

            // Wait for a message to arrive from the host stream (TODO: or for the stream to close)
            while let Some(msg) = stream.next().await {
                // Wait for the guest to signal that it's ready
                let state = future::poll_fn(|context| {
                    let mut host_streams = host_streams.lock().unwrap();

                    if host_streams.closed_streams.contains(&new_stream_id) {
                        // Stream has closed
                        host_streams.closed_streams.remove(&new_stream_id);
                        host_streams.ready_streams.remove(&new_stream_id);

                        Poll::Ready(State::Closed)
                    } else if host_streams.ready_streams.contains(&new_stream_id) {
                        // Stream is ready for more data
                        host_streams.ready_streams.remove(&new_stream_id);

                        Poll::Ready(State::Ready)
                    } else {
                        // Wake us up when the stream state changes
                        host_streams.stream_waker.insert(new_stream_id, Some(context.waker().clone()));

                        Poll::Pending
                    }
                }).await;

                match state {
                    State::Closed => { break; }

                    State::Ready => {
                        // Guest is ready to receive the message, send it on via the actions
                        if guest_actions.send(GuestAction::SendStream(new_stream_id, msg)).await.is_err() {
                            // Can no longer send guest actions to anything
                            break;
                        }
                    }
                }
            }
        };

        // Add our future to the pile to start it running
        self.2.add_future(run_stream);

        // Result is this new stream ID
        Ok(new_stream_id)
    }

    fn receive_stream(&self, stream_id: SerializationId) -> Result<BoxStream<'static, Vec<u8>>, SceneSendError<SerializationId>> {
        // Create a guest stream for this stream
        let new_stream_core = GuestStreamCore {
            pending:    VecDeque::new(),
            closed:     false,
            waker:      None
        };
        let new_stream_core = Arc::new(Mutex::new(new_stream_core));

        // Add to the guest streams so the host can wake us once the 
        self.0.lock().unwrap().guest_streams.insert(stream_id, new_stream_core.clone());

        // Create a results stream
        let actions = self.1.clone();

        let stream = stream::unfold((actions, new_stream_core), move |(actions, stream_core)| async move {
            use std::mem;
            use futures::task::{Poll};

            // TODO: arrange for a 'closed' message to be sent if this is ever dropped
            let mut actions = actions;

            loop {
                // Return the next message if there are messages waiting
                {
                    let mut locked_core = stream_core.lock().unwrap();

                    if let Some(next_message) = locked_core.pending.pop_front() {
                        // There is a pending message
                        mem::drop(locked_core);
                        return Some((next_message, (actions, stream_core)));
                    } else if locked_core.closed {
                        // The stream has closed and there are no remaining messages to deliver
                        return None;
                    }
                }

                // Stream is ready, we're waiting for a new message
                actions.send(GuestAction::ReadyStream(stream_id)).await.ok();

                // Wait for something to wake us up
                let stream_core = stream_core.clone();
                future::poll_fn(move |ctxt| {
                    let mut locked_core = stream_core.lock().unwrap();

                    if !locked_core.closed && locked_core.pending.is_empty() {
                        locked_core.waker = Some(ctxt.waker().clone());
                        Poll::Pending
                    } else {
                        Poll::Ready(())
                    }
                }).await;
            }
        });

        Ok(stream.boxed())
    }

    fn send_function(&self, callback: RemoteCallbackFn) -> Result<SerializationId, SceneSendError<RemoteCallbackFn>> {
        todo!()
    }

    fn receive_function(&self, callback_id: SerializationId) -> Result<RemoteCallbackFn, SceneSendError<SerializationId>> {
        todo!()
    }
}

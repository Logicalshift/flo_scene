use super::guest_context::*;
use super::guest_stream_core::*;
use super::poll_action::*;
use super::poll_result::*;
use super::input_stream::*;
use super::sink_handle::*;
use super::stream_id::*;
use super::stream_target::*;
use super::subprogram_handle::*;
use crate::guest::guest_serialization_context::GuestSerializationContext;
use crate::host::error::*;
use crate::host::scene_message::*;
use crate::host::serialization_context::*;
use crate::host::subprogram_id::*;
use crate::util::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{waker, ArcWake, Context, Poll, Waker};
use futures::channel::mpsc;

use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::sync::*;

pub (crate) struct GuestRuntimeCore {
    /// The runner for the futures in this core (None while we're polling it)
    future_runner: Option<BoxFuture<'static, ()>>,

    /// The future pile, which can be used to schedule new futures on this core
    pub (super) future_pile: FuturePile,

    /// Set to true if the waker has been triggered for anything in the future pile
    pile_is_awake: bool,

    /// The input stream cores used in the runtime
    input_streams: HashMap<usize, Arc<Mutex<GuestInputStreamCore>>>,

    /// Sink handles
    sink_handles: HashMap<usize, GuestSink>,

    /// The handle to assign to the next input stream we assign
    next_stream_handle: usize,

    /// The handle to assign to the next sink that we create
    next_sink_handle: usize,

    /// Actions and results that are waiting to be returned to the host
    pub (super) pending_results: Vec<GuestResult>,

    /// The next ID to assign to a serializatble stream on the guest side
    next_serialization_id: usize,

    /// The streams that are marked as ready on the host side
    pub (super) ready_streams: HashSet<SerializationId>,

    /// The streams that are marked closed (and which still exist on the guest side)
    pub (super) closed_streams: HashSet<SerializationId>,

    /// Wakers to notify when a stream becomes ready or is closed
    pub (super) when_ready: HashMap<SerializationId, Option<Waker>>,

    /// The streams with pending data from the host side
    pending_streams: HashMap<SerializationId, Arc<Mutex<GuestStreamCore>>>,
}

///
/// The guest runtime runs a set of guest subprograms (providing GuestInputStream and GuestSceneContext functions),
/// and also supplies the functions that process GuestActions and generate GuestResults. From the point of view of
/// the guest subprograms, it's a single-threaded futures executor.
///
pub struct GuestRuntime {
    /// The core, which manages the runtime
    core: Arc<Mutex<GuestRuntimeCore>>,
}

impl GuestRuntime {
    ///
    /// Creates a new guest runtime with the specified subprogram
    ///
    /// The initial subprogram always has GuestSubProgramHandle(0) for sending input to (this is also `GuestSubProgramHandle::default`).
    ///
    /// The subprogram ID here is only used to generate the initialisation message for this default subprogram.
    ///
    pub fn with_default_subprogram<TMessageType, TFuture>(program_id: SubProgramId, subprogram: impl FnOnce(GuestInputStream<TMessageType>, GuestSceneContext) -> TFuture) -> Self 
    where
        TMessageType:   SceneMessage,
        TFuture:        'static + Send + Future<Output=()>,
    {
        // Create the runtime
        let (pile, runner)      = FuturePile::new();
        let future_pile         = pile.clone();
        let pile_is_awake       = true;
        let future_runner       = Some(runner.run_forever().boxed());
        let input_streams       = HashMap::new();
        let sink_handles        = HashMap::new();
        let next_stream_handle  = 0;
        let next_sink_handle    = 0;
        let next_serialization_id = 0;
        let program_handle      = GuestSubProgramHandle::default();
        let pending_results     = vec![GuestResult::CreateSubprogram(program_id, program_handle, HostStreamId::for_message::<TMessageType>())];
        let ready_streams       = HashSet::new();
        let closed_streams      = HashSet::new();
        let when_ready          = HashMap::new();
        let pending_streams     = HashMap::new();

        let core = GuestRuntimeCore { future_runner, future_pile, pile_is_awake, input_streams, sink_handles, next_stream_handle, next_sink_handle, next_serialization_id, pending_results, ready_streams, closed_streams, when_ready, pending_streams };
        let core = Arc::new(Mutex::new(core));

        let runtime             = GuestRuntime { core: Arc::clone(&core) };
        let serialization_ctxt  = GuestSerializationContext::new(&core, &pile);

        // Initialise the initial subprogram
        let (_input_handle, input_stream)   = runtime.create_input_stream();
        let context                         = GuestSceneContext { core: Arc::clone(&core), subprogram_id: program_id, serialization_context: serialization_ctxt };
        let subprogram                      = subprogram(input_stream, context);
        let subprogram                      = async move {
            // Run the program
            subprogram.await;

            // Wait for anything else in the future pile to become idle (in particular, to ensure that any streams finish processing)
            let future_pile = core.lock().unwrap().future_pile.clone();
            future_pile.idle().await;

            // Notify that it has finished (adding to the results means that the runtime will pick up the message later on)
            core.lock().unwrap().pending_results.push(GuestResult::EndedSubprogram(program_handle));

            // TODO: we don't know for sure the core has stopped here, except that there's currently no way to create another subprogram
            core.lock().unwrap().pending_results.push(GuestResult::Stopped);
        };

        pile.add_future(subprogram);
        debug_assert!(_input_handle == 0);

        runtime
    }

    ///
    /// Creates a guest input stream in this runtime, returning the stream and the handle for the stream
    ///
    #[inline]
    pub fn create_input_stream<TMessageType: SceneMessage>(&self) -> (usize, GuestInputStream<TMessageType>) {
        GuestRuntimeCore::create_input_stream(&self.core)
    }

    ///
    /// Polls any awake futures in this scene, returning any resulting actions
    ///
    /// In general, guest programs should be inherently non-blocking and isolated from anything running in the 'parent' context
    /// so calling this from an existing future should generally be safe.
    ///
    #[inline]
    pub fn poll_awake(&self) -> Vec<GuestResult> {
        GuestRuntimeCore::poll_awake(&self.core)
    }

    ///
    /// Enqueues a messge for the specified subprogram
    ///
    /// This will always accept the message, but the specified subprogram should be considered 'not ready' after this call has
    /// been made so that backpressure is generated. The message is discarded if there is no subprogram with the specified
    /// ID running
    ///
    pub fn send_message(&self, target: GuestSubProgramHandle, data: Vec<u8>) {
        GuestRuntimeCore::send_message(&self.core, target, data)
    }

    ///
    /// Flags that a sink is ready to receive data
    ///
    pub fn sink_ready(&self, HostSinkHandle(sink): HostSinkHandle) {
        let waker = {
            let mut core = self.core.lock().unwrap();

            if let Some(sink_data) = core.sink_handles.get_mut(&sink) {
                // Set the sink to ready and wake it up
                sink_data.status = GuestSinkStatus::Ready;
                sink_data.waker.take()
            } else {
                // No sink with this handle is available
                None
            }
        };

        // Wake up the future for later polling
        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Indicates that a sink could not be connected
    ///
    pub fn sink_connection_error(&self, HostSinkHandle(sink): HostSinkHandle, error: ConnectionError) {
        let waker = {
            let mut core = self.core.lock().unwrap();

            if let Some(sink_data) = core.sink_handles.get_mut(&sink) {
                // Set the sink to the error state
                sink_data.status = GuestSinkStatus::ConnectionError(error);
                sink_data.waker.take()
            } else {
                // No sink with this handle is available
                None
            }
        };

        // Wake up the future for later polling
        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Indicates that a message could not be sent on a sink
    ///
    pub fn sink_send_error(&self, HostSinkHandle(sink): HostSinkHandle, error: SceneSendError<Vec<u8>>) {
        let waker = {
            let mut core = self.core.lock().unwrap();

            if let Some(sink_data) = core.sink_handles.get_mut(&sink) {
                // Set the sink to the error state
                sink_data.status = GuestSinkStatus::SendError(error);
                sink_data.waker.take()
            } else {
                // No sink with this handle is available
                None
            }
        };

        // Wake up the future for later polling
        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Sends a message from the host to the guest
    ///
    pub fn send_stream(&self, stream_id: SerializationId, msg: Vec<u8>) {
        // Fetch the stream we're sending to, or create a new one (assuming that the stream will get created later on: we rely on the host not to send more data to a stream once it is woken up)
        let stream = {
            let mut core = self.core.lock().unwrap();

            core.pending_streams.entry(stream_id)
                .or_insert_with(|| Arc::new(Mutex::new(GuestStreamCore { 
                    pending:    VecDeque::new(), 
                    waker:      None, 
                    closed:     false 
                })))
                .clone()
        };

        // Push data to it and wake it up
        let waker = {
            let mut core = stream.lock().unwrap();

            core.pending.push_back(msg);
            core.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }

    ///
    /// A host stream is ready to receive more data
    ///
    pub fn ready_stream(&self, stream_id: SerializationId) {
        // Add this stream ID to the list that's 'ready', and wake up anything that's waiting
        let waker = {
            let mut core = self.core.lock().unwrap();

            core.ready_streams.insert(stream_id);
            core.when_ready.get_mut(&stream_id)
                .map(|waker| waker.take())
                .unwrap_or(None)
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }

    ///
    /// A host or guest stream has been closed by the host
    ///
    /// If the stream ID refers to a guest stream, the host has closed the stream. If it's a guest stream, the receiver
    /// has been dropped.
    ///
    pub fn close_stream(&self, stream_id: SerializationId) {
        use std::mem;

        // Add this stream ID to the list that's 'ready', and wake up anything that's waiting
        let waker = {
            let mut core = self.core.lock().unwrap();

            if let Some(guest_stream) = core.pending_streams.get_mut(&stream_id).cloned() {
                // Remove the guest stream and then mark it as closed
                core.pending_streams.remove(&stream_id);
                mem::drop(core);

                // Need to lock the guest stream after releasing the core (which does create a window where the guest stream is not in the core and not closed)
                let mut guest_stream = guest_stream.lock().unwrap();
                guest_stream.closed = true;
                guest_stream.waker.take()
            } else {
                // If it's not a guest stream, must be a host stream: mark it as deleted
                core.closed_streams.insert(stream_id);
                core.when_ready.get_mut(&stream_id)
                    .map(|waker| waker.take())
                    .unwrap_or(None)
            }
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }

    ///
    /// Processes a single action in this runtime (note that `poll_awake()` needs to be called after this to actually execute the runtime)
    ///
    pub fn process(&self, action: GuestAction) {
        use GuestAction::*;

        match action {
            SendMessage(sub_program, message)       => { self.send_message(sub_program, message) }
            Ready(sink_handle)                      => { self.sink_ready(sink_handle) },
            SinkConnectionError(sink_handle, error) => { self.sink_connection_error(sink_handle, error) },
            SinkError(sink_handle, error)           => { self.sink_send_error(sink_handle, error) }
            SendStream(stream_id, msg)              => { self.send_stream(stream_id.invert(), msg) },
            ReadyStream(stream_id)                  => { self.ready_stream(stream_id.invert()) },
            CloseStream(stream_id)                  => { self.close_stream(stream_id.invert()) },
        }
    }

    ///
    /// Creates a sender/receiver pair from this runtime that will run the guest runtime
    ///
    /// The caller can read actions from the returned stream, and send actions to the sender (which is an mpsc sender
    /// so can be replicated if there are multiple sources of actions if needed)
    ///
    pub fn as_streams(self) -> (mpsc::Sender<GuestAction>, impl 'static + Send + Unpin + Stream<Item=GuestResult>) {
        // Create the sender/receiver
        let (action_sender, action_receiver) = mpsc::channel(32);

        // We gather the receiver values into chunks to process as many as possible at once
        let action_receiver = action_receiver.ready_chunks(64);

        // Poll the runtime to make sure that it's in an idle condition
        let initial_results     = self.poll_awake();
        let stopped             = false;
        let poll_immediately    = false;

        // Create the result stream; the runtime is run by awaiting on this
        let result_stream = stream::unfold((self, action_receiver, stopped, poll_immediately), |(runtime, action_receiver, stopped, poll_immediately)| async move {
            let mut action_receiver = action_receiver;

            if stopped {
                // Most recent poll result indicated we have run out of actions (we have to wait to stop the stream as we want the results to be processed)
                return None;
            }

            let maybe_actions = if poll_immediately {
                // The guest indicated it wanted an immediate callback without waiting (so we do so once all of the results have been processed)
                Some(vec![])
            } else {
                // The guest is idle, so we wait until some external action wakes it up
                action_receiver.next().await
            };

            // Process the actions in the guest
            if let Some(actions) = maybe_actions {
                // Process the actions into the runtime
                actions.into_iter().for_each(|action| runtime.process(action));

                // Poll for the next set of results
                let next_actions = runtime.poll_awake();

                // Check if the runtime has stopped or if we need to poll immediately the next time through
                let mut stopped             = stopped;
                let mut poll_immediately    = false;

                for action in next_actions.iter() {
                    match action {
                        GuestResult::Stopped            => { stopped = true;}
                        GuestResult::ContinuePolling    => { poll_immediately = true; }
                        _                               => { }
                    }
                }

                // Convert to a stream
                let next_actions = stream::iter(next_actions);
                Some((next_actions, (runtime, action_receiver, stopped, poll_immediately)))
            } else {
                // The actions have finished
                None
            }
        }).flatten();

        // Chain the initial results with the extra result stream
        let result_stream = stream::iter(initial_results).chain(result_stream);

        // Result is the stream we just built
        (action_sender, Box::pin(result_stream))
    }
}

struct CoreWaker(Weak<Mutex<GuestRuntimeCore>>);

impl ArcWake for CoreWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        if let Some(runtime_core) = arc_self.0.upgrade() {
            // If the core still exists, add this future to the awake list
            let mut core = runtime_core.lock().unwrap();
            core.pile_is_awake = true;
        }
    }
}

impl GuestRuntimeCore {
    ///
    /// Creates a new input stream in a runtime core
    ///
    pub (crate) fn create_input_stream<TMessageType: SceneMessage>(runtime_core: &Arc<Mutex<Self>>) -> (usize, GuestInputStream<TMessageType>) {
        let mut core = runtime_core.lock().unwrap();

        // Assign a handle to the input stream
        let stream_handle = core.next_stream_handle;
        core.next_stream_handle += 1;

        // Create a new serialization context for the core
        let serialization_context = GuestSerializationContext::new(&runtime_core, &core.future_pile);

        // Create a core for the new stream
        let input_stream    = GuestInputStream::new(GuestSubProgramHandle(stream_handle), runtime_core, serialization_context);
        let input_core      = input_stream.core().clone();

        core.input_streams.insert(stream_handle, input_core);

        (stream_handle, input_stream)
    }

    ///
    /// Polls any awake futures in this core
    ///
    #[inline]
    pub (crate) fn poll_awake(core: &Arc<Mutex<Self>>) -> Vec<GuestResult> {
        use std::mem;

        // TODO: need to mark the futures as stopped and finished
        loop {
            // Fetch the runner from the core (we borrow it while it's active)
            let future_runner = {
                let mut core = core.lock().unwrap();

                if core.pile_is_awake {
                    core.pile_is_awake = false;
                    core.future_runner.take()
                } else {
                    None
                }
            };

            if let Some(mut future_runner) = future_runner {
                // The waker sets the core as 'awake' if it's woken (main reason for it is to go through this loop again if we get re-awoken while polling)
                let core_waker  = CoreWaker(Arc::downgrade(core));
                let core_waker  = waker(Arc::new(core_waker));
                let mut context = Context::from_waker(&core_waker);

                // Run the futures that are awake (we ignore the result because we know we use poll_forever)
                let _ = future_runner.poll_unpin(&mut context);

                // Return the runner to the core so we're ready for the next pass through the loop
                (*core.lock().unwrap()).future_runner = Some(future_runner);
            } else {
                // Return the results if there's nothing to poll
                if future_runner.is_none() {
                    let mut core    = core.lock().unwrap();
                    let mut results = vec![];
                    mem::swap(&mut results, &mut core.pending_results);

                    return results;
                }
            }
        }
    }

    ///
    /// Enqueues a messge for the specified subprogram
    ///
    /// This will always accept the message, but the specified subprogram should be considered 'not ready' after this call has
    /// been made so that backpressure is generated. The message is discarded if there is no subprogram with the specified
    /// ID running
    ///
    pub (crate) fn send_message(core: &Arc<Mutex<Self>>, target: GuestSubProgramHandle, message: Vec<u8>) {
        use std::mem;

        let waker = {
            // Lock the core
            let core = core.lock().unwrap();

            // The handle is an index into the input_streams list
            let GuestSubProgramHandle(target_id) = target;

            // Get the input stream, if we can
            let input_stream = core.input_streams.get(&target_id).cloned();

            // Release the lock on the core
            mem::drop(core);

            if let Some(input_stream) = input_stream {
                GuestInputStreamCore::send_message(&input_stream, message)
            } else {
                // This program is not running
                None
            }
        };

        // Wake anything that needs to be awoken for this stream
        waker.into_iter()
            .for_each(|waker| waker.wake());
    }

    ///
    /// Indicates that a stream is ready to accept more input
    ///
    pub (crate) fn stream_ready(core: &Arc<Mutex<Self>>, target: GuestSubProgramHandle) {
        // Indicate that the program is ready to receive a new message
        let mut core = core.lock().unwrap();

        core.pending_results.push(GuestResult::Ready(target))
    }

    ///
    /// Performs a request to open a sink on the host side
    ///
    pub (crate) fn open_host_sink(core: &Arc<Mutex<Self>>, target: HostStreamTarget) -> impl Send + Future<Output=Result<HostSinkHandle, ConnectionError>> {
        let core = Arc::clone(core);

        // Create a new sink. It's only a proposed sink handle at this point as we'll throw it away if it errors out
        let proposed_sink_handle = {
            let mut core = core.lock().unwrap();
            let handle   = core.next_sink_handle;

            core.sink_handles.insert(handle, GuestSink { waker: None, status: GuestSinkStatus::Busy });
            core.next_sink_handle += 1;

            handle
        };

        // Queue a request for this stream
        core.lock().unwrap().pending_results.push(GuestResult::Connect(HostSinkHandle(proposed_sink_handle), target));

        // Poll until the sink moves to the ready state
        future::poll_fn(move |context| {
            let mut core = core.lock().unwrap();

            if let Some(sink_data) = core.sink_handles.get_mut(&proposed_sink_handle) {
                match &sink_data.status {
                    GuestSinkStatus::Busy => {
                        // Sink is still waiting for data
                        sink_data.waker = Some(context.waker().clone());
                        Poll::Pending
                    }

                    GuestSinkStatus::Ready => {
                        // Sink is ready to send data
                        Poll::Ready(Ok(HostSinkHandle(proposed_sink_handle)))
                    }

                    GuestSinkStatus::ConnectionError(error) => {
                        // Sink could not connect
                        let error = error.clone();
                        core.sink_handles.remove(&proposed_sink_handle);
                        Poll::Ready(Err(error))
                    }

                    GuestSinkStatus::SendError(_error) => {
                        // Unexpected error as we're not trying to send anything to the sink at this point
                        core.sink_handles.remove(&proposed_sink_handle);
                        Poll::Ready(Err(ConnectionError::Cancelled))
                    }
                }
            } else {
                // Sink disappeared while we were waiting
                Poll::Ready(Err(ConnectionError::Cancelled))
            }
        })
    }

    ///
    /// Sends an encoded message to a host sink
    ///
    pub (crate) fn send_to_host_sink(core: &Arc<Mutex<Self>>, sink: HostSinkHandle, message: Vec<u8>) -> impl Send + Unpin + Future<Output=Result<(), SceneSendError<Vec<u8>>>> {
        let core = Arc::clone(core);

        // Poll until the sink moves to the ready state
        let mut message = Some(message);
        let HostSinkHandle(sink) = sink;

        future::poll_fn(move |context| {
            let mut core = core.lock().unwrap();

            if let Some(sink_data) = core.sink_handles.get_mut(&sink) {
                match &sink_data.status {
                    GuestSinkStatus::Busy => {
                        // Sink is still waiting for data
                        sink_data.waker = Some(context.waker().clone());
                        Poll::Pending
                    }

                    GuestSinkStatus::Ready => {
                        if let Some(message) = message.take() {
                            // Move the sink to the busy state
                            sink_data.status = GuestSinkStatus::Busy;
                            sink_data.waker  = Some(context.waker().clone());

                            // Send the data
                            core.pending_results.push(GuestResult::Send(HostSinkHandle(sink), message));

                            // Wait for the sink to become ready (or report an error)
                            Poll::Pending
                        } else {
                            // Message was previously sent and the sink is now ready again
                            Poll::Ready(Ok(()))
                        }
                    }

                    GuestSinkStatus::ConnectionError(_error) => {
                        // Unexpected error
                        panic!("Connection error (stream should already be connected");
                    }

                    GuestSinkStatus::SendError(error) => {
                        // Unexpected error as we're not trying to send anything to the sink at this point
                        let error = error.clone();
                        core.sink_handles.remove(&sink);
                        Poll::Ready(Err(error))
                    }
                }
            } else {
                // Sink disappeared while we were waiting
                Poll::Ready(Err(SceneSendError::TargetProgramEndedBeforeReady))
            }
        })
    }

    ///
    /// Creates a sink that receives encoded data and sends it to a target 
    ///
    pub (crate) fn create_output_sink(core: &Arc<Mutex<Self>>, target: HostStreamTarget) -> impl Future<Output=Result<impl 'static + Send + Unpin + Sink<Vec<u8>, Error=SceneSendError<Vec<u8>>>, ConnectionError>> {
        let core = Arc::clone(&core);

        async move {
            // Create the connection to the core
            let sink_handle = GuestRuntimeCore::open_host_sink(&core, target).await?;

            // Use unfold to send messages
            Ok(sink::unfold((), move |_, data| GuestRuntimeCore::send_to_host_sink(&core, sink_handle, data)))
        }
    }

    ///
    /// Creates a guest stream core for reading data for a stream located on the host
    ///
    pub (crate) fn create_stream_from_host(core: &Arc<Mutex<Self>>, stream_id: SerializationId) -> Arc<Mutex<GuestStreamCore>> {
        // Create a guest core to represent this stream
        let stream_core = GuestStreamCore {
            pending:    VecDeque::new(),
            waker:      None,
            closed:     false,
        };
        let stream_core = Arc::new(Mutex::new(stream_core));

        // Store the new stream in the core (or if an exising one is already present, substitute that one)
        core.lock().unwrap().pending_streams.entry(stream_id)
            .or_insert(stream_core)
            .clone()
    }

    ///
    /// Creates a serialization ID for a stream on the guest side
    ///
    pub (crate) fn next_serialization_id(core: &Arc<Mutex<Self>>) -> SerializationId {
        let mut core    = core.lock().unwrap();
        let next_id     = core.next_serialization_id;
        core.next_serialization_id += 1;

        SerializationId::MyStream(next_id)
    }
}

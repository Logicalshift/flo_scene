use super::guest_context::*;
use super::guest_message::*;
use super::poll_action::*;
use super::poll_result::*;
use super::input_stream::*;
use super::sink_handle::*;
use super::stream_target::*;
use super::subprogram_handle::*;
use crate::host::error::*;
use crate::host::subprogram_id::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{waker, ArcWake, Context, Poll};
use futures::channel::mpsc;

use std::collections::{HashMap, HashSet};
use std::sync::*;

///
/// Enum representing the state of a future in the guest runtime
///
enum GuestFuture {
    /// Future is ready to run
    Ready(BoxFuture<'static, ()>),

    /// Future is being polled elsewhere
    Busy,

    /// Future is finished (and can be replaced by another future if needed)
    Finished
}

pub (crate) struct GuestRuntimeCore {
    /// The futures that are running in the guest
    futures: Vec<GuestFuture>,

    /// Indices of the futures that are awake
    awake: HashSet<usize>,

    /// The input stream cores used in the runtime
    input_streams: HashMap<usize, Arc<Mutex<GuestInputStreamCore>>>,

    /// Sink handles
    sink_handles: HashMap<usize, GuestSink>,

    /// The handle to assign to the next input stream we assign (which doubles as the )
    next_stream_handle: usize,

    /// The handle to assign to the next
    next_sink_handle: usize,

    /// Actions and results that are waiting to be returned to the host
    pending_results: Vec<GuestResult>,
}

/// Wakes up the future with the specified index in a guest runtime core
struct CoreWaker(usize, Weak<Mutex<GuestRuntimeCore>>);

///
/// The guest runtime runs a set of guest subprograms (providing GuestInputStream and GuestSceneContext functions),
/// and also supplies the functions that process GuestActions and generate GuestResults. From the point of view of
/// the guest subprograms, it's a single-threaded futures executor.
///
pub struct GuestRuntime<TEncoder: GuestMessageEncoder> {
    /// The core, which manages the runtime
    core: Arc<Mutex<GuestRuntimeCore>>,

    /// The encoder, used for serializing and deserializing messages sent to and from the guest program
    encoder: TEncoder,
}

impl<TEncoder> GuestRuntime<TEncoder>
where
    TEncoder: 'static + GuestMessageEncoder,
{
    ///
    /// Creates a new guest runtime with the specified subprogram
    ///
    /// The initial subprogram always has GuestSubProgramHandle(0) for sending input to (this is also `GuestSubProgramHandle::default`).
    ///
    /// The subprogram ID here is only used to generate the initialisation message for this default subprogram.
    ///
    pub fn with_default_subprogram<TMessageType, TFuture>(program_id: SubProgramId, encoder: TEncoder, subprogram: impl FnOnce(GuestInputStream<TMessageType>, GuestSceneContext<TEncoder>) -> TFuture) -> Self 
    where
        TMessageType:   GuestSceneMessage,
        TFuture:        'static + Send + Future<Output=()>,
    {
        // Create the runtime
        let futures             = vec![];
        let awake               = HashSet::new();
        let input_streams       = HashMap::new();
        let sink_handles        = HashMap::new();
        let next_stream_handle  = 0;
        let next_sink_handle    = 0;
        let pending_results     = vec![GuestResult::CreateSubprogram(program_id, GuestSubProgramHandle::default(), TMessageType::stream_id())];

        let core = GuestRuntimeCore { futures, awake, input_streams, sink_handles, next_stream_handle, next_sink_handle, pending_results };
        let core = Arc::new(Mutex::new(core));

        let context_encoder = encoder.clone();
        let runtime         = GuestRuntime { core: Arc::clone(&core), encoder };

        // Initialise the initial subprogram
        let (_input_handle, input_stream)   = runtime.create_input_stream();
        let context                         = GuestSceneContext { core: Arc::clone(&core), encoder: context_encoder, subprogram_id: program_id };
        let subprogram                      = subprogram(input_stream, context);

        core.lock().unwrap().futures.push(GuestFuture::Ready(subprogram.boxed()));
        core.lock().unwrap().awake.insert(0);
        debug_assert!(_input_handle == 0);

        runtime
    }

    ///
    /// Creates a guest input stream in this runtime, returning the stream and the handle for the stream
    ///
    #[inline]
    pub fn create_input_stream<TMessageType: GuestSceneMessage>(&self) -> (usize, GuestInputStream<TMessageType>) {
        GuestRuntimeCore::create_input_stream(&self.core, &self.encoder)
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
    /// Processes a single action in this runtime (note that `poll_awake()` needs to be called after this to actually execute the runtime)
    ///
    pub fn process(&self, action: GuestAction) {
        use GuestAction::*;

        match action {
            SendMessage(sub_program, message)       => { self.send_message(sub_program, message) }
            Ready(sink_handle)                      => { self.sink_ready(sink_handle) },
            SinkConnectionError(sink_handle, error) => { self.sink_connection_error(sink_handle, error) },
            SinkError(sink_handle, error)           => { self.sink_send_error(sink_handle, error) }
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
        let initial_results = self.poll_awake();

        // Create the result stream; the runtime is run by awaiting on this
        let result_stream = stream::unfold((self, action_receiver), |(runtime, action_receiver)| async move {
            let mut action_receiver = action_receiver;

            if let Some(actions) = action_receiver.next().await {
                // Process the actions into the runtime
                actions.into_iter().for_each(|action| runtime.process(action));

                // Poll for the next set of results
                let next_actions = runtime.poll_awake();
                let next_actions = stream::iter(next_actions);

                Some((next_actions, (runtime, action_receiver)))
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

impl GuestFuture {
    ///
    /// If this future is in the ready state, returns Some(future) and leaves this in the busy state 
    ///
    #[inline]
    pub fn take(&mut self) -> Option<BoxFuture<'static, ()>> {
        use std::mem;

        match self {
            GuestFuture::Ready(_) => {
                let mut taken_future = GuestFuture::Busy;
                mem::swap(self, &mut taken_future);

                match taken_future {
                    GuestFuture::Ready(taken_future)    => Some(taken_future),
                    _                                   => unreachable!()
                }
            }

            _ => None
        }
    }
}

impl ArcWake for CoreWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let CoreWaker(future_idx, weak_runtime_core) = &**arc_self;
        let future_idx = *future_idx;

        if let Some(runtime_core) = weak_runtime_core.upgrade() {
            // If the core still exists, add this future to the awake list
            let mut core = runtime_core.lock().unwrap();
            core.awake.insert(future_idx);
        }
    }
}

impl GuestRuntimeCore {
    ///
    /// Creates a new input stream in a runtime core
    ///
    pub (crate) fn create_input_stream<TMessageType: GuestSceneMessage>(runtime_core: &Arc<Mutex<Self>>, encoder: &(impl 'static + GuestMessageEncoder)) -> (usize, GuestInputStream<TMessageType>) {
        let mut core = runtime_core.lock().unwrap();

        // Assign a handle to the input stream
        let stream_handle = core.next_stream_handle;
        core.next_stream_handle += 1;

        // Create a core for the new stream
        let input_stream    = GuestInputStream::new(GuestSubProgramHandle(stream_handle), encoder.clone(), runtime_core);
        let input_core      = input_stream.core().clone();

        core.input_streams.insert(stream_handle, input_core);

        (stream_handle, input_stream)
    }

    ///
    /// Polls any awake futures in this core
    ///
    pub (crate) fn poll_awake(core: &Arc<Mutex<Self>>) -> Vec<GuestResult> {
        use std::mem;

        loop {
            // Pick the futures to poll
            let ready_to_poll = {
                // Take all of the futures that are ready out of the core (and mark them as asleep again)
                let mut core    = core.lock().unwrap();
                let core        = &mut *core;

                let awake   = &mut core.awake;
                let futures = &mut core.futures;

                awake.drain()
                    .flat_map(|idx| {
                        futures[idx].take().map(|future| (idx, future))
                    })
                    .collect::<Vec<_>>()
            };

            // Return the actions that were generated when there are no more futures ready to run
            if ready_to_poll.is_empty() {
                // Take the pending results out of the core
                let results = {
                    let mut core    = core.lock().unwrap();
                    let mut results = vec![];
                    mem::swap(&mut results, &mut core.pending_results);

                    results
                };

                // Finished polling the futures
                return results;
            }

            // Poll the futures
            // TODO: (stopping if we build up enough results)
            for (future_idx, ready_future) in ready_to_poll.into_iter() {
                // Create a context to poll in (will wake the attached future if hit)
                let core_waker  = CoreWaker(future_idx, Arc::downgrade(core));
                let core_waker  = waker(Arc::new(core_waker));
                let mut context = Context::from_waker(&core_waker);

                // Poll the future
                let mut ready_future = ready_future;
                let poll_result = ready_future.poll_unpin(&mut context);

                // Return the future to the list (or mark it as finished)
                match poll_result {
                    Poll::Pending => { 
                        core.lock().unwrap().futures[future_idx] = GuestFuture::Ready(ready_future);
                    }

                    Poll::Ready(_) => { 
                        let mut core = core.lock().unwrap();
                        core.futures[future_idx] = GuestFuture::Finished; 
                        core.pending_results.push(GuestResult::EndedSubprogram(GuestSubProgramHandle(future_idx)));
                    }
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
}

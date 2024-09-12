use super::guest_message::*;
use super::poll_result::*;
use super::input_stream::*;
use super::sink_handle::*;
use super::stream_id::*;
use super::stream_target::*;
use super::subprogram_handle::*;
use crate::host::error::*;
use crate::host::subprogram_id::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{waker, ArcWake, Context, Poll};

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

/// Wakes up future with the specified index in a guest runtime core
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

///
/// A guest scene context relays requests from the guest side to the host side
///
pub struct GuestSceneContext;

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
    pub fn with_default_subprogram<TMessageType, TFuture>(program_id: SubProgramId, encoder: TEncoder, subprogram: impl FnOnce(GuestInputStream<TMessageType>, GuestSceneContext) -> TFuture) -> Self 
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

        let runtime = GuestRuntime { core: Arc::clone(&core), encoder };

        // Initialise the initial subprogram
        let (_input_handle, input_stream)   = runtime.create_input_stream();
        let context                         = GuestSceneContext;
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
    pub (crate) fn send_to_host_sink(core: &Arc<Mutex<Self>>, sink: HostSinkHandle, message: Vec<u8>) -> impl Send + Future<Output=Result<HostSinkHandle, SceneSendError<Vec<u8>>>> {
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
        core.lock().unwrap().pending_results.push(GuestResult::Send(sink, message));

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

                    GuestSinkStatus::ConnectionError(_error) => {
                        // Unexpected error
                        panic!("Connection error (stream should already be connected");
                    }

                    GuestSinkStatus::SendError(error) => {
                        // Unexpected error as we're not trying to send anything to the sink at this point
                        let error = error.clone();
                        core.sink_handles.remove(&proposed_sink_handle);
                        Poll::Ready(Err(error))
                    }
                }
            } else {
                // Sink disappeared while we were waiting
                Poll::Ready(Err(SceneSendError::TargetProgramEndedBeforeReady))
            }
        })
    }
}

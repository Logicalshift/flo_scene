use crate::errors::*;
use crate::guest_types::*;
use crate::host_types::*;
use crate::guest_result::*;
use crate::util::*;
use super::input_stream::*;
use super::input_stream_core::*;
use super::serialization_context::*;
use super::stream_core::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{Waker, Poll, Context, ArcWake, waker};

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::sync::*;
use alloc::vec::*;

pub (crate) struct GuestRuntimeCore {
    /// The runner for the futures in this core (None while we're polling it)
    future_runner: Option<BoxFuture<'static, ()>>,

    /// The future pile, which can be used to schedule new futures on this core
    pub (super) future_pile: FuturePile,

    /// Set to true if the waker has been triggered for anything in the future pile
    pile_is_awake: bool,

    /// The input stream cores used in the runtime
    input_streams: BTreeMap<usize, Shared<GuestInputStreamCore>>,

    /// Sink handles
    pub (super) sink_handles: BTreeMap<usize, GuestSink>,

    /// The handle to assign to the next input stream we assign
    next_stream_handle: usize,

    /// The handle to assign to the next sink that we create
    next_sink_handle: usize,

    /// Actions and results that are waiting to be returned to the host
    pub (super) pending_results: Vec<GuestResult>,

    /// The next ID to assign to a serializatble stream on the guest side
    next_serialization_id: usize,

    /// The streams that are marked as ready on the host side
    pub (super) ready_streams: BTreeSet<SerializationId>,

    /// The streams that are marked closed (and which still exist on the guest side)
    pub (super) closed_streams: BTreeSet<SerializationId>,

    /// Wakers to notify when a stream becomes ready or is closed
    pub (super) when_ready: BTreeMap<SerializationId, Option<Waker>>,

    /// The streams with pending data from the host side
    pub (super) pending_streams: BTreeMap<SerializationId, Shared<GuestStreamCore>>,
}

impl GuestRuntimeCore {
    ///
    /// Creates a new empty core
    ///
    pub (crate) fn new() -> Self {
        let (pile, runner)      = FuturePile::new();
        let future_pile         = pile;
        let pile_is_awake       = true;
        let future_runner       = Some(runner.run_forever().boxed());
        let input_streams       = BTreeMap::new();
        let sink_handles        = BTreeMap::new();
        let next_stream_handle  = 0;
        let next_sink_handle    = 0;
        let next_serialization_id = 0;
        let pending_results     = Vec::new();
        let ready_streams       = BTreeSet::new();
        let closed_streams      = BTreeSet::new();
        let when_ready          = BTreeMap::new();
        let pending_streams     = BTreeMap::new();

        let core = GuestRuntimeCore { future_runner, future_pile, pile_is_awake, input_streams, sink_handles, next_stream_handle, next_sink_handle, next_serialization_id, pending_results, ready_streams, closed_streams, when_ready, pending_streams };

        core
    }

    ///
    /// Creates a new input stream in a runtime core
    ///
    pub (crate) fn create_input_stream<TMessageType: SceneGuestMessage>(runtime_core: &Shared<Self>) -> (usize, GuestInputStream<TMessageType>) {
        with_shared(&runtime_core, |core| {
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
        })
    }

    ///
    /// Polls any awake futures in this core
    ///
    #[inline]
    pub (crate) fn poll_awake(core: &Shared<Self>) -> Vec<GuestResult> {
        use core::mem;

        // TODO: need to mark the futures as stopped and finished
        loop {
            // Fetch the runner from the core (we borrow it while it's active)
            let future_runner = with_shared(core, |core| {
                if core.pile_is_awake {
                    core.pile_is_awake = false;
                    core.future_runner.take()
                } else {
                    None
                }
            });

            if let Some(mut future_runner) = future_runner {
                // The waker sets the core as 'awake' if it's woken (main reason for it is to go through this loop again if we get re-awoken while polling)
                let core_waker  = CoreWaker(shared_downgrade(core));
                let core_waker  = waker(Arc::new(core_waker));
                let mut context = Context::from_waker(&core_waker);

                // Run the futures that are awake (we ignore the result because we know we use poll_forever)
                let _ = future_runner.poll_unpin(&mut context);

                // Return the runner to the core so we're ready for the next pass through the loop
                with_shared(core, |core| core.future_runner = Some(future_runner));
            } else {
                // Return the results if there's nothing to poll
                if future_runner.is_none() {
                    return with_shared(core, |core| {
                        let mut results = Vec::new();
                        mem::swap(&mut results, &mut core.pending_results);
                        results
                    });
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
    pub (crate) fn send_message(core: &Shared<Self>, target: GuestSubProgramHandle, message: Vec<u8>) {
        let input_stream = with_shared(core, |core| {
            // The handle is an index into the input_streams list
            let GuestSubProgramHandle(target_id) = target;

            // Get the input stream, if we can
            core.input_streams.get(&target_id).cloned()
        });

        let waker = if let Some(input_stream) = input_stream {
            GuestInputStreamCore::send_message(&input_stream, message)
        } else {
            // This program is not running
            None
        };

        // Wake anything that needs to be awoken for this stream
        waker.into_iter()
            .for_each(|waker| waker.wake());
    }

    ///
    /// Indicates that a stream is ready to accept more input
    ///
    pub (crate) fn stream_ready(core: &Shared<Self>, target: GuestSubProgramHandle) {
        // Indicate that the program is ready to receive a new message
        with_shared(core, |core| {
            core.pending_results.push(GuestResult::Ready(target))
        })
    }

    ///
    /// Performs a request to open a sink on the host side
    ///
    pub (crate) fn open_host_sink(core: &Shared<Self>, target: HostStreamTarget) -> impl Send + Future<Output=Result<HostSinkHandle, ConnectionError>> {
        let core = core.clone();

        // Create a new sink. It's only a proposed sink handle at this point as we'll throw it away if it errors out
        let proposed_sink_handle = with_shared(&core, |core| {
            let handle   = core.next_sink_handle;

            core.sink_handles.insert(handle, GuestSink { waker: None, status: GuestSinkStatus::Busy });
            core.next_sink_handle += 1;

            handle
        });

        // Queue a request for this stream
        with_shared(&core, |core| core.pending_results.push(GuestResult::Connect(HostSinkHandle(proposed_sink_handle), target)));

        // Poll until the sink moves to the ready state
        future::poll_fn(move |context| with_shared(&core, |core| {
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
        }))
    }

    ///
    /// Sends an encoded message to a host sink
    ///
    pub (crate) fn send_to_host_sink(core: &Shared<Self>, sink: HostSinkHandle, message: Vec<u8>) -> impl Send + Unpin + Future<Output=Result<(), SceneSendError<Vec<u8>>>> {
        let core = core.clone();

        // Poll until the sink moves to the ready state
        let mut message = Some(message);
        let HostSinkHandle(sink) = sink;

        future::poll_fn(move |context| with_shared(&core, |core| {
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

                    GuestSinkStatus::ConnectionError(error) => {
                        // Unexpected error
                        Poll::Ready(Err(SceneSendError::CouldNotConnect(error.clone())))
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
        }))
    }

    ///
    /// Creates a sink that receives encoded data and sends it to a target 
    ///
    pub (crate) fn create_output_sink(core: &Shared<Self>, target: HostStreamTarget) -> impl Future<Output=Result<impl 'static + Send + Unpin + Sink<Vec<u8>, Error=SceneSendError<Vec<u8>>>, ConnectionError>> {
        let core = core.clone();

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
    pub (crate) fn create_stream_from_host(core: &Shared<Self>, stream_id: SerializationId) -> Shared<GuestStreamCore> {
        // Create a guest core to represent this stream
        let stream_core = GuestStreamCore {
            pending:    VecDeque::new(),
            waker:      None,
            closed:     false,
        };
        let stream_core = share(stream_core);

        // Store the new stream in the core (or if an exising one is already present, substitute that one)
        with_shared(core, |core| {
            core.pending_streams.entry(stream_id)
                .or_insert(stream_core)
                .clone()
        })
    }

    ///
    /// Creates a serialization ID for a stream on the guest side
    ///
    pub (crate) fn next_serialization_id(core: &Shared<Self>) -> SerializationId {
        with_shared(core, |core| {
            let next_id = core.next_serialization_id;
            core.next_serialization_id += 1;

            SerializationId::MyStream(next_id)
        })
    }
}

struct CoreWaker(WeakShared<GuestRuntimeCore>);

impl ArcWake for CoreWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        with_weak_shared(&arc_self.0, |core| {
            // Future pile should have been woken up
            core.pile_is_awake = true;
        });
    }
}

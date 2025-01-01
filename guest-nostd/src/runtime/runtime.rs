use crate::errors::*;
use crate::guest_types::*;
use crate::host_types::*;
use super::context::*;
use super::core::*;
use super::input_stream::*;
use super::serialization_context::*;
use super::stream_core::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::task::{Waker};

use alloc::boxed::*;
use alloc::collections::{VecDeque};
use alloc::vec::*;

///
/// The guest runtime runs a set of guest subprograms (providing GuestInputStream and GuestSceneContext functions),
/// and also supplies the functions that process GuestActions and generate GuestResults. From the point of view of
/// the guest subprograms, it's a single-threaded futures executor.
///
pub struct GuestRuntime {
    /// The core, which manages the runtime
    core: Shared<GuestRuntimeCore>,
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
        TMessageType:   SceneGuestMessage,
        TFuture:        'static + Send + Future<Output=()>,
    {
        // Create the runtime
        let mut core        = GuestRuntimeCore::new();
        let pile            = core.future_pile.clone();
        let program_handle  = GuestSubProgramHandle::default();

        // Indicate that the subprogram is starting
        core.pending_results.push(GuestResult::CreateSubprogram(program_id, program_handle, HostStreamId::for_message::<TMessageType>()));

        let core = share(core);

        let runtime             = GuestRuntime { core: core.clone() };
        let serialization_ctxt  = GuestSerializationContext::new(&core, &pile);

        // Initialise the initial subprogram
        let (_input_handle, input_stream)   = runtime.create_input_stream();
        let context                         = GuestSceneContext { core: core.clone(), subprogram_id: program_id, serialization_context: serialization_ctxt };
        let subprogram                      = subprogram(input_stream, context);
        let subprogram                      = async move {
            // Run the program
            subprogram.await;

            // Wait for anything else in the future pile to become idle (in particular, to ensure that any streams finish processing)
            let future_pile = with_shared(&core, |core| core.future_pile.clone());
            future_pile.idle().await;

            with_shared(&core, |core| {
                // Notify that it has finished (adding to the results means that the runtime will pick up the message later on)
                core.pending_results.push(GuestResult::EndedSubprogram(program_handle));

                // TODO: we don't know for sure the core has stopped here, except that there's currently no way to create another subprogram
                core.pending_results.push(GuestResult::Stopped);
            });
        };

        pile.add_future(subprogram);
        debug_assert!(_input_handle == 0);

        runtime
    }

    ///
    /// Creates a guest input stream in this runtime, returning the stream and the handle for the stream
    ///
    #[inline]
    pub fn create_input_stream<TMessageType: SceneGuestMessage>(&self) -> (usize, GuestInputStream<TMessageType>) {
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
        let waker = with_shared(&self.core, |core| {
            if let Some(Some(sink_data)) = core.sink_handles.get_mut(sink) {
                // Set the sink to ready and wake it up
                sink_data.status = GuestSinkStatus::Ready;
                sink_data.waker.take()
            } else {
                // No sink with this handle is available
                None
            }
        });

        // Wake up the future for later polling
        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Indicates that a sink could not be connected
    ///
    pub fn sink_connection_error(&self, HostSinkHandle(sink): HostSinkHandle, error: ConnectionError) {
        let waker = with_shared(&self.core, |core| {
            if let Some(Some(sink_data)) = core.sink_handles.get_mut(sink) {
                // Set the sink to the error state
                sink_data.status = GuestSinkStatus::ConnectionError(error);
                sink_data.waker.take()
            } else {
                // No sink with this handle is available
                None
            }
        });

        // Wake up the future for later polling
        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Indicates that a message could not be sent on a sink
    ///
    pub fn sink_send_error(&self, HostSinkHandle(sink): HostSinkHandle, error: SceneSendError<Vec<u8>>) {
        let waker = with_shared(&self.core, |core| {
            if let Some(Some(sink_data)) = core.sink_handles.get_mut(sink) {
                // Set the sink to the error state
                sink_data.status = GuestSinkStatus::SendError(error);
                sink_data.waker.take()
            } else {
                // No sink with this handle is available
                None
            }
        });

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
        let stream = with_shared(&self.core, |core| {
            core.pending_streams.entry(stream_id)
                .or_insert_with(|| share(GuestStreamCore { 
                    pending:    VecDeque::new(), 
                    waker:      None, 
                    closed:     false 
                }))
                .clone()
        });

        // Push data to it and wake it up
        let waker = with_shared(&stream, |core| {
            core.pending.push_back(msg);
            core.waker.take()
        });

        if let Some(waker) = waker {
            waker.wake();
        }
    }

    ///
    /// A host stream is ready to receive more data
    ///
    pub fn ready_stream(&self, stream_id: SerializationId) {
        // Add this stream ID to the list that's 'ready', and wake up anything that's waiting
        let waker = with_shared(&self.core, |core| {
            core.ready_streams.insert(stream_id, ());
            core.when_ready.get_mut(&stream_id)
                .map(|waker| waker.take())
                .unwrap_or(None)
        });

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
        enum CloseAction {
            None,
            Wake(Waker),
            CloseGuestStream(Shared<GuestStreamCore>),
        }

        // Add this stream ID to the list that's 'ready', and wake up anything that's waiting
        let action = with_shared(&self.core, |core| {
            if let Some(guest_stream) = core.pending_streams.get_mut(&stream_id).cloned() {
                // Remove the guest stream and then mark it as closed
                core.pending_streams.remove(&stream_id);

                // Need to lock the guest stream after releasing the core (which does create a window where the guest stream is not in the core and not closed)
                CloseAction::CloseGuestStream(guest_stream)
            } else {
                // If it's not a guest stream, must be a host stream: mark it as deleted
                core.closed_streams.insert(stream_id, ());
                let waker = core.when_ready.get_mut(&stream_id)
                    .map(|waker| waker.take())
                    .unwrap_or(None);

                match waker {
                    None        => CloseAction::None,
                    Some(waker) => CloseAction::Wake(waker),
                }
            }
        });

        match action {
            CloseAction::None           => { }
            CloseAction::Wake(waker)    => { waker.wake(); }

            CloseAction::CloseGuestStream(guest_stream) => {
                let waker = with_shared(&guest_stream, |guest_stream| {
                    guest_stream.closed = true;
                    guest_stream.waker.take()
                });

                if let Some(waker) = waker {
                    waker.wake();
                }
            }
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
                Some(Vec::new())
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


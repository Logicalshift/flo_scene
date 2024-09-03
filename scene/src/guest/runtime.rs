use super::guest_message::*;
use super::poll_result::*;
use super::input_stream::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{waker, ArcWake, Context, Poll};

use std::collections::{HashMap, HashSet};
use std::marker::{PhantomData};
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

struct GuestRuntimeCore {
    /// The futures that are running in the guest
    futures: Vec<GuestFuture>,

    /// Indices of the futures that are awake
    awake: HashSet<usize>,

    /// The input stream cores used in the runtime
    input_streams: HashMap<usize, Arc<Mutex<GuestInputStreamCore>>>,

    /// The handle to assign to the next input stream we assign
    next_stream_handle: usize,
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
    /// The initial subprogram always has GuestSubProgramHandle(0) for sending input to
    ///
    pub fn with_default_subprogram<TMessageType, TFuture>(encoder: TEncoder, subprogram: impl FnOnce(GuestInputStream<TMessageType>, GuestSceneContext) -> TFuture) -> Self 
    where
        TMessageType:   GuestSceneMessage,
        TFuture:        'static + Send + Future<Output=()>,
    {
        // Create the runtime
        let futures             = vec![];
        let awake               = HashSet::new();
        let input_streams       = HashMap::new();
        let next_stream_handle  = 0;

        let core = GuestRuntimeCore { futures, awake, input_streams, next_stream_handle };
        let core = Arc::new(Mutex::new(core));

        let runtime = GuestRuntime { core: Arc::clone(&core), encoder };

        // Initialise the initial subprogram
        let (_input_handle, input_stream)   = runtime.create_input_stream();
        let context                         = GuestSceneContext;
        let subprogram                      = subprogram(input_stream, context);

        core.lock().unwrap().futures.push(GuestFuture::Ready(subprogram.boxed()));
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
    pub (crate) fn create_input_stream<TMessageType: GuestSceneMessage>(core: &Arc<Mutex<Self>>, encoder: &(impl 'static + GuestMessageEncoder)) -> (usize, GuestInputStream<TMessageType>) {
        let mut core = core.lock().unwrap();

        // Assign a handle to the input stream
        let stream_handle = core.next_stream_handle;
        core.next_stream_handle += 1;

        // Create a core for the new stream
        let input_stream    = GuestInputStream::new(encoder.clone());
        let input_core      = input_stream.core().clone();

        core.input_streams.insert(stream_handle, input_core);

        (stream_handle, input_stream)
    }

    ///
    /// Polls any awake futures in this core
    ///
    pub (crate) fn poll_awake(core: &Arc<Mutex<Self>>) -> Vec<GuestResult> {
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

            // Return the actions that were generated when there aren o
            if ready_to_poll.is_empty() {
                // TODO: collect pending messages from the futures to return here
                return vec![];
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
                    Poll::Ready(_) => { core.lock().unwrap().futures[future_idx] = GuestFuture::Finished; }
                    Poll::Pending  => { core.lock().unwrap().futures[future_idx] = GuestFuture::Ready(ready_future); }
                }
            }
        }
    }
}

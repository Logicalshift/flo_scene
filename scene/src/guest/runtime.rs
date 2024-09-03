use super::guest_message::*;
use super::poll_result::*;
use super::input_stream::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{Waker};

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
    /// If set_context is true, this will set a futures context. This panics if called from another context, so the flag can be
    /// set to false if the existing context should be used. (Things will also work with no context at all: the main thing that
    /// the futures context does is panic if you try to enter another one)
    ///
    /// In general, guest programs should be inherently non-blocking and isolated from anything running in the 'parent' context
    /// so calling this from an existing future with set_context set to false should generally be safe.
    ///
    #[inline]
    pub fn poll_awake(&self, set_context: bool) -> Vec<GuestResult> {
        GuestRuntimeCore::poll_awake(&self.core, set_context)
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
    pub (crate) fn poll_awake(core: &Arc<Mutex<Self>>, set_context: bool) -> Vec<GuestResult> {
        // Pick the futures to poll

        // Poll the futures (stopping if we build up enough results)

        // Return the polled futures to the core

        // Return any results that were generated while polling
        vec![]
    }
}

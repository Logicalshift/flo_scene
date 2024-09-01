use super::guest_message::*;
use super::input_stream::*;

use futures::prelude::*;
use futures::future::{BoxFuture};
use futures::task::{Waker};

use std::collections::{HashSet};
use std::marker::{PhantomData};
use std::sync::*;

struct GuestRuntimeCore<TEncoder: GuestMessageEncoder> {
    /// The futures that are running in the guest
    futures: Vec<Option<BoxFuture<'static, ()>>>,

    /// The encoder, used for serializing and deserializing messages sent to and from the guest program
    encoder: TEncoder,

    /// Indices of the futures that are awake
    awake: HashSet<usize>,

    /// The input stream cores used in the runtime
    input_streams: Vec<Option<Arc<Mutex<GuestInputStreamCore>>>>,
}

///
/// The guest runtime runs a set of guest subprograms (providing GuestInputStream and GuestSceneContext functions),
/// and also supplies the functions that process GuestActions and generate GuestResults. From the point of view of
/// the guest subprograms, it's a single-threaded futures executor.
///
pub struct GuestRuntime<TEncoder: GuestMessageEncoder> {
    core: Arc<Mutex<GuestRuntimeCore<TEncoder>>>,
}

///
/// A guest scene context relays requests from the guest side to the host side
///
pub struct GuestSceneContext;

impl<TEncoder> GuestRuntime<TEncoder>
where
    TEncoder: GuestMessageEncoder,
{
    ///
    /// Creates a new guest runtime with the specified subprogram
    ///
    pub fn with_default_subprogram<TMessageType, TFuture>(encoder: TEncoder, subprogram: impl FnOnce(GuestInputStream<TMessageType>, GuestSceneContext)) -> Self 
    where
        TMessageType:   GuestSceneMessage,
        TFuture:        'static + Send + Future<Output=()>,
    {
        // Create the runtime
        let futures         = vec![];
        let awake           = HashSet::new();
        let input_streams   = vec![];

        let core = GuestRuntimeCore { futures, encoder, awake, input_streams };
        let core = Arc::new(Mutex::new(core));

        let runtime = GuestRuntime { core: Arc::clone(&core) };

        // TODO: initialise the initial subprogram

        runtime
    }
}

use crate::guest_types::*;
use crate::host_types::*;
use crate::guest_result::*;
use crate::util::*;

use futures::future::{BoxFuture};
use futures::task::{Waker};

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::*;

pub (crate) struct GuestRuntimeCore {
    /// The runner for the futures in this core (None while we're polling it)
    future_runner: Option<BoxFuture<'static, ()>>,

    /// The future pile, which can be used to schedule new futures on this core
    pub (super) future_pile: FuturePile,

    /// Set to true if the waker has been triggered for anything in the future pile
    pile_is_awake: bool,

    /// The input stream cores used in the runtime
    // input_streams: BTreeMap<usize, Arc<Mutex<GuestInputStreamCore>>>,

    /// Sink handles
    // sink_handles: BTreeMap<usize, GuestSink>,

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
    // pending_streams: BTreeSet<SerializationId, Arc<Mutex<GuestStreamCore>>>,
    nothing: ()
}

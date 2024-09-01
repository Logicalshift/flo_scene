use crate::scene_message::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::collections::{HashSet};
use std::marker::{PhantomData};

///
/// The guest runtime runs a set of guest subprograms (providing GuestInputStream and GuestSceneContext functions),
/// and also supplies the functions that process GuestActions and generate GuestResults. From the point of view of
/// the guest subprograms, it's a single-threaded futures executor.
///
pub struct GuestRuntime {
    /// The futures that are running in the guest
    futures: Vec<Option<BoxFuture<'static, ()>>>,

    /// Indices of the futures that are awake
    awake: HashSet<usize>,
}

///
/// A guest input stream works with the reads deserialized messages from the host side
///
pub struct GuestInputStream<TMessageType>(PhantomData<TMessageType>);

///
/// A guest scene context relays requests from the guest side to the host side
///
pub struct GuestSceneContext;

impl GuestRuntime {
    ///
    /// Creates a new guest runtime with the specified subprogram
    ///
    pub fn with_default_subprogram<TMessageType, TFuture>(subprogram: impl FnOnce(GuestInputStream<TMessageType>, GuestSceneContext)) -> Self 
    where
        TMessageType:   SceneMessage,
        TFuture:        'static + Send + Future<Output=()>,
    {
        todo!();
    }
}

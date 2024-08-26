use crate::scene_message::*;

use futures::prelude::*;

use std::marker::{PhantomData};

///
/// The guest runtime runs a set of guest subprograms (providing GuestInputStream and GuestSceneContext functions),
/// and also supplies the functions that process GuestActions and generate GuestResults. From the point of view of
/// the guest subprograms, it's a single-threaded futures executor.
///
pub struct GuestRuntime {

}

pub struct GuestInputStream<TMessageType>(PhantomData<TMessageType>);
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

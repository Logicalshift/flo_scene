use crate::error::*;
use crate::context::*;

use futures::prelude::*;

///
/// Extension methods for futures in a scene context
///
pub trait SceneFutureExt {
    ///
    /// Runs this future in the background of the active entity
    ///
    fn run_in_background(self) -> Result<(), EntityFutureError>;
}

impl<T> SceneFutureExt for T
where
    T: 'static + Send + Future<Output=()>,
{
    fn run_in_background(self) -> Result<(), EntityFutureError> {
        SceneContext::current().run_in_background(self)
    }
}

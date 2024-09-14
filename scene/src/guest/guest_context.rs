use super::runtime::*;
use crate::host::*;

use std::sync::*;

///
/// A guest scene context relays requests from the guest side to the host side
///
pub struct GuestSceneContext {
    pub (crate) core: Arc<Mutex<GuestRuntimeCore>>,
}

impl GuestSceneContext {
    pub fn current_program_id(&self) -> Option<SubProgramId> {
        todo!();
    }

    pub fn send<TMessageType>(&self, target: impl Into<StreamTarget>) -> Result<OutputSink<TMessageType>, ConnectionError>
    where
        TMessageType: 'static + SceneMessage,
    {
        todo!()
    }

    pub async fn send_message<TMessageType>(&self, message: TMessageType) -> Result<(), ConnectionError> 
    where
        TMessageType: 'static + SceneMessage,
    {
        todo!()
    }

    // TODO: guest versions of spawn_command and spawn_query (these are a bit complicated, so duplicating them is a pain: suggests that
    // improving the design might make things easier, ideally want to make use of the host's implementation I think)
}

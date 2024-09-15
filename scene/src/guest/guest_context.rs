use super::guest_message::*;
use super::runtime::*;
use super::stream_target::*;
use crate::host::*;

use futures::prelude::*;

use std::sync::*;

///
/// A guest scene context relays requests from the guest side to the host side
///
pub struct GuestSceneContext {
    /// The core of the runtime where this context is running
    pub (crate) core: Arc<Mutex<GuestRuntimeCore>>,
}

impl GuestSceneContext {
    pub fn current_program_id(&self) -> Option<SubProgramId> {
        todo!();
    }

    // TODO: use StreamTarget here
    pub fn send<TMessageType>(&self, target: impl Into<HostStreamTarget>) -> Result<impl Sink<TMessageType, Error=SceneSendError<Vec<u8>>>, ConnectionError>
    where
        TMessageType: 'static + SceneMessage + GuestSceneMessage,
    {
        // Set up the state
        let connection  = None;
        let mut core    = Some(self.core.clone());
        let target      = target.into();

        Ok(sink::unfold(connection, move |connection, item| {
            match connection {
                None => {
                    // Create a new connection before sending the message
                    let core    = core.take().unwrap();
                    let target  = target.clone();

                    async move {
                        // Create the connection
                        let connection = GuestRuntimeCore::create_output_sink(&core, target).await;

                        match connection {
                            Ok(mut connection) => {
                                // TODO: encode the message (need to pass in the encoder)
                                let encoded = vec![];

                                // Send the encoded message
                                connection.send(encoded).await?;

                                Ok(Some(connection))
                            }

                            Err(err) => {
                                Err(SceneSendError::CouldNotConnect(err))
                            }
                        }
                    }.boxed()
                }

                Some(mut connection) => {
                    // Connection already exists, so send the message
                    async move {
                        // TODO: encode the message
                        // let encoded = vec![];

                        // Send the encoded message
                        // TODO: Rust can't figure out the type (it should be able to because it can with just the above code, but apparently not... which is an issue because it's anonymous so we can't declare it)
                        // connection.send(encoded).await?;

                        Ok(Some(connection))
                    }.boxed()
                }
            }
        }))
    }

    /*
    pub async fn send_message<TMessageType>(&self, message: TMessageType) -> Result<(), ConnectionError> 
    where
        TMessageType: 'static + SceneMessage + GuestSceneMessage,
    {
        let mut target = self.send::<TMessageType>(())?;

        target.send(message).await?
    }
    */

    // TODO: guest versions of spawn_command and spawn_query (these are a bit complicated, so duplicating them is a pain: suggests that
    // improving the design might make things easier, ideally want to make use of the host's implementation I think)
}

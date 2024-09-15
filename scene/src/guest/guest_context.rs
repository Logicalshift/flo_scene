use super::guest_message::*;
use super::runtime::*;
use super::stream_target::*;
use crate::host::*;

use futures::prelude::*;

use std::sync::*;

///
/// A guest scene context relays requests from the guest side to the host side
///
pub struct GuestSceneContext<TEncoder> {
    /// The ID of the program running in this context
    pub (crate) subprogram_id: SubProgramId,

    /// The core of the runtime where this context is running
    pub (crate) core: Arc<Mutex<GuestRuntimeCore>>,

    /// Used to encode messages
    pub (crate) encoder: TEncoder,
}

impl<TEncoder> GuestSceneContext<TEncoder>
where
    TEncoder: 'static + GuestMessageEncoder,
{
    ///
    /// Returns the currently active subprogram, if there is one
    ///
    /// This will return 'None' if the scene that the program was running in is terminated but the
    /// task is still running, so this is a very rare occurrence. 
    ///
    pub fn current_program_id(&self) -> Option<SubProgramId> {
        Some(self.subprogram_id)
    }

    ///
    /// Retrieves a stream for sending messages of the specified type
    ///
    /// The target can be used to define the default destination for the stream. If the target is a specific program, that program should
    /// have an input type that matches the message type. If the target is `None` or `Any`, the stream can be connected by the scene (by the
    /// `connect_programs()` request), so the exact target does not need to be known.
    ///
    /// The `None` target will discard any messages received while the stream is disconnected, but the `Any` target will block until something
    /// connects the stream. Streams with a specified target will connect to that target immediately.
    ///
    pub fn send<TMessageType>(&self, target: impl Into<StreamTarget>) -> Result<impl Unpin + Sink<TMessageType, Error=SceneSendError<Vec<u8>>>, ConnectionError>
    where
        TMessageType: 'static + SceneMessage + GuestSceneMessage,
    {
        // Set up the state
        let connection  = None;
        let core        = Some(self.core.clone());
        let target      = Some(HostStreamTarget::from_stream_target::<TMessageType>(target)?);
        let encoder     = self.encoder.clone();

        Ok(sink::unfold((connection, core, target, encoder), move |(connection, core, target, encoder), item| {
            Box::pin(async move {
                let mut connection = match connection {
                    None => {
                        let core    = core.unwrap();
                        let target  = target.unwrap();

                        // Create the connection
                        let connection = GuestRuntimeCore::create_output_sink(&core, target).await;

                        match connection {
                            Ok(connection) => {
                                connection
                            }

                            Err(err) => {
                                return Err(SceneSendError::CouldNotConnect(err));
                            }
                        }
                    }


                    Some(connection) => connection,
                };

                // Encode the message
                let encoded = encoder.encode(item);

                // Send the encoded message
                // TODO: Rust can't figure out the type (it should be able to because it can with just the above code, but apparently not... which is an issue because it's anonymous so we can't declare it)
                connection.send(encoded).await?;

                Ok((Some(connection), None, None, encoder))
            })
        }))
    }

    ///
    /// Sends a single message to the default output of that type
    ///
    pub async fn send_message<TMessageType>(&self, message: TMessageType) -> Result<(), ConnectionError> 
    where
        TMessageType: 'static + SceneMessage + GuestSceneMessage,
    {
        let mut target = self.send::<TMessageType>(())?;

        target.send(message).await?;

        Ok(())
    }

    // TODO: guest versions of spawn_command and spawn_query (these are a bit complicated, so duplicating them is a pain: suggests that
    // improving the design might make things easier, ideally want to make use of the host's implementation I think)
}

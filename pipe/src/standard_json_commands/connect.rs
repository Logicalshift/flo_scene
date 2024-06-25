use crate::commands::*;

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;

///
/// Indicates the source or target program of a connection
///
#[derive(Clone, Serialize, Deserialize)]
pub enum Connection {
    /// Indicates that a stream should connect to no program (ie, discard all messages). Creates no connection if used on the source side.
    None,

    /// On the source side, connect from any producer of this type of stream. As a target, connect to whatever is configured as the default for this stream
    Any,

    /// Connection to/from a specific subprogram
    Program(SubProgramId),
}

///
/// The arguments to the connect command
///
#[derive(Clone, Serialize, Deserialize)]
pub struct ConnectArguments {
    /// The program that is producing the stream as an output. 'None' will create no connection, and 'Any' will update everything that has this stream type
    source_program: Connection,

    /// The program that the data should be sent to. 'None' indicates that data for this stream should be discarded, and 'Any' indicates that the data should
    /// be sent to any stream
    target_program: Connection,

    /// The 'serialized type name' of the stream that's being connected (Rust type names are not allowed here as they may change from one version to the next)
    stream_type_name: String,
}

///
///
///
#[derive(Clone, Serialize, Deserialize)]
pub enum ConnectResponse {
    /// Connection was made OK
    Ok,

    /// An error was encountered
    Error(ConnectionError),
}

///
/// The `connect` command, which connects two subprograms in a scene
///
pub fn command_connect(input: ConnectArguments, context: SceneContext) -> impl Future<Output=CommandResponseData<ConnectResponse>> {
    async move {
        // Parse the source and target
        let source = match &input.source_program {
            Connection::None                => { return CommandResponseData::Data(ConnectResponse::Ok); },
            Connection::Any                 => StreamSource::All,
            Connection::Program(prog_id)    => StreamSource::Program(*prog_id),
        };

        let target = match &input.target_program {
            Connection::None                => StreamTarget::None,
            Connection::Any                 => StreamTarget::Any,
            Connection::Program(prog_id)    => StreamTarget::Program(*prog_id),
        };

        // Stream ID must use a serialization  name
        let stream_id = StreamId::with_serialization_type(&input.stream_type_name);
        let stream_id = if let Some(stream_id) = stream_id { stream_id } else { return ConnectResponse::Error(ConnectionError::StreamNotKnown).into(); };

        // Send a scene control request
        if let Err(err) = context.send_message(SceneControl::connect(source, target, stream_id)).await {
            return ConnectResponse::Error(err).into();
        }

        // Seems OK
        ConnectResponse::Ok.into()
    }
}

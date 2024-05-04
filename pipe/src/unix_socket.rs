use flo_scene::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use std::path::*;
use std::os::unix::net::{UnixListener};

///
/// Creates a sub-program that accepts connections on a unix domain socket that binds at a specified path
///
/// To use this subprogram, the scene must be running inside a tokio runtime.
///
pub fn create_unix_socket_program<TInputStream, TOutputStream>(scene: &Scene, program_id: SubProgramId, path: impl AsRef<Path>, 
    create_input_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, u8>) -> TInputStream,
    create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, u8>) -> TOutputStream) 
    -> Result<(), ConnectionError> 
where
    TInputStream:   'static + Send + Stream,
    TOutputStream:  'static + Send + Stream,
{
    // TODO
    Err(ConnectionError::TargetNotAvailable)
}

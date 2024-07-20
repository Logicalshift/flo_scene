use super::socket::*;

use flo_scene::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

#[cfg(unix)]
use tokio::net::{UnixListener};

use std::path::*;
use std::sync::*;

///
/// Starts a sub-program in a sceme that accepts connections on a unix domain socket that binds at a specified path
///
/// To use this subprogram, the scene must be running inside a tokio runtime. The program will accept no connections if this
/// crate was not compiled for UNIX.
///
/// The program will wait for subscribers (the `Subscribe` message) to the `SocketMessage<TInputStream::Item, TOutputStream::Item>`
/// message. Typically, there's only one subscriber but in the event multiple are connected, they are informed of connections in
/// a round-robin fashion.
///
pub fn start_unix_socket_program<TInputStream, TOutputMessage>(
        scene:                  &Scene, 
        program_id:             SubProgramId, 
        path:                   impl AsRef<Path>, 
        create_input_messages:  impl 'static + Send + Sync + Fn(BoxStream<'static, Vec<u8>>) -> TInputStream,
        create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, TOutputMessage>) -> BoxStream<'static, Vec<u8>>
    ) -> Result<(), ConnectionError> 
where
    TInputStream:   'static + Send + Stream,
    TOutputMessage: 'static + Send,
{
    #[cfg(unix)]
    {
        // Create the listener for this program
        let listener = UnixListener::bind(path)
            .map_err(|tokio_err| ConnectionError::IoError(format!("{}", tokio_err)))?;
        let listener = Arc::new(Mutex::new(Some(listener)));

        // Add a socket runner subprogram. We don't use the address for anything, ie we accept all connections here
        scene.add_subprogram(program_id, move |_input: InputStream<()>, context| socket_listener_subprogram(context, move || {
                let listener        = Arc::clone(&listener);
                let our_listener    = listener.lock().unwrap().take().unwrap();

                async move {
                    let connection = our_listener.accept().await
                        .map(|(socket, _addr)| socket.into_split())
                        .map_err(|tokio_err| tokio_err.into());

                    *listener.lock().unwrap() = Some(our_listener);

                    connection
                }
            },
            create_input_messages,
            create_output_messages), 0);

        // Success
        Ok(())
    }

    #[cfg(not(unix))]
    {
        // If we're not on Unix, this creates a program that ignores its messages (we can't create any UNIX sockets)
        scene.add_subprogram(program_id, move |input: InputStream<()>, _context| async move {
            let mut input = input;
            while let Some(_) = input.next().await {
            }
        }, 0);

        Ok(())
    }
}

use super::socket::*;

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use tokio::net::{TcpListener, ToSocketAddrs};

use ::desync::*;

use std::path::*;

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
pub fn start_unencrpted_tcp_socket<TInputStream, TOutputMessage>(
        scene: &Scene, 
        program_id: SubProgramId, 
        address: impl 'static + Send + ToSocketAddrs, 
        create_input_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, Vec<u8>>) -> TInputStream,
        create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, TOutputMessage>) -> BoxStream<'static, Vec<u8>>
    ) -> Result<(), ConnectionError> 
where
    TInputStream:   'static + Send + Stream,
    TOutputMessage: 'static + Send,
{
    #[cfg(unix)]
    {
        scene.add_subprogram(program_id, move |input, context| async move {
            // The listener requires an await to start, so we create it as part of the program
            let listener = TcpListener::bind(address).await
                .map_err(|tokio_err| ConnectionError::IoError(format!("{}", tokio_err)))
                .unwrap();

            // Add a socket runner subprogram. We don't use the address for anything, ie we accept all connections here
            let listener = Desync::new(listener);

            socket_listener_subprogram(input.messages_with_sources(), context, move || 
                listener.future_desync(|listener| async {
                    listener.accept().await
                        .map(|(socket, _addr)| {
                            socket.set_nodelay(false).ok();
                            socket.into_split()
                        })
                        .map_err(|tokio_err| tokio_err.into())
                }).map_ok_or_else(|_cancelled| Err(ConnectionError::Cancelled), |ok| ok),
                create_input_messages,
                create_output_messages).await;
            }, 0);

        // Success
        Ok(())
    }

    #[cfg(not(unix))]
    {
        // If we're not on Unix, this creates a program that ignores its messages (we can't create any UNIX sockets)
        scene.add_subprogram(program_id, move |input: InputStream<Subscribe>, _context| async move {
            let mut input = input;
            while let Some(_) = input.next().await {
            }
        }, 0);

        Ok(())
    }
}

use super::socket::*;

use flo_scene::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use tokio::net::{TcpListener, ToSocketAddrs};

use ::desync::*;

///
/// Starts a sub-program that accepts unencrypted connections on a TCP socket.
///
/// The program will wait for subscribers (the `Subscribe` message) to the `SocketMessage<TInputStream::Item, TOutputStream::Item>`
/// message. Typically, there's only one subscriber but in the event multiple are connected, they are informed of connections in
/// a round-robin fashion.
///
pub fn start_unencrpted_tcp_socket<TInputStream, TOutputMessage>(
        scene:                  &Scene, 
        program_id:             SubProgramId, 
        address:                impl 'static + Send + ToSocketAddrs, 
        create_input_messages:  impl 'static + Send + Sync + Fn(BoxStream<'static, Vec<u8>>) -> TInputStream,
        create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, TOutputMessage>) -> BoxStream<'static, Vec<u8>>
    ) -> Result<(), ConnectionError> 
where
    TInputStream:   'static + Send + Stream,
    TOutputMessage: 'static + Send,
{
    scene.add_subprogram(program_id, move |input, context| async move {
        // The listener requires an await to start, so we create it as part of the program
        let listener = TcpListener::bind(address).await
            .map_err(|tokio_err| ConnectionError::IoError(format!("{}", tokio_err)))
            .unwrap();

        // Add a socket runner subprogram. We don't use the address for anything, ie we accept all connections here
        let listener = Desync::new(listener);

        socket_listener_subprogram(input, context, move || 
            listener.future_desync(|listener| async {
                listener.accept().await
                    .map(|(socket, _addr)| {
                        socket.set_nodelay(true).ok();
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

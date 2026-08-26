use super::socket::*;

use flo_scene::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use tokio::net::{TcpListener, ToSocketAddrs};

use std::sync::*;

///
/// Starts a sub-program that accepts unencrypted connections on a TCP socket.
///
/// The program will wait for subscribers (the `Subscribe` message) to the `SocketMessage<TInputStream::Item, TOutputStream::Item>`
/// message. Typically, there's only one subscriber but in the event multiple are connected, they are informed of connections in
/// a round-robin fashion.
///
pub fn start_unencrypted_tcp_socket<TInputStream, TOutputMessage>(
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
    scene.add_subprogram(program_id, move |_input: InputStream<()>, context| unencrypted_tcp_socket_subprogram(context, address, create_input_messages, create_output_messages), 0);

    // Success
    Ok(())
}

///
/// Implements a sub-program that accepts unencrypted connections on a TCP socket.
///
/// The program will wait for subscribers (the `Subscribe` message) to the `SocketMessage<TInputStream::Item, TOutputStream::Item>`
/// message. Typically, there's only one subscriber but in the event multiple are connected, they are informed of connections in
/// a round-robin fashion.
///
pub async fn unencrypted_tcp_socket_subprogram<TInputStream, TOutputMessage>(
        context:                SceneContext,
        address:                impl 'static + Send + ToSocketAddrs, 
        create_input_messages:  impl 'static + Send + Sync + Fn(BoxStream<'static, Vec<u8>>) -> TInputStream,
        create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, TOutputMessage>) -> BoxStream<'static, Vec<u8>>
    )
where
    TInputStream:   'static + Send + Stream,
    TOutputMessage: 'static + Send,
{
    // The listener requires an await to start, so we create it as part of the program
    let listener = TcpListener::bind(address).await
        .map_err(|tokio_err| ConnectionError::IoError(format!("{}", tokio_err)))
        .unwrap();
    let listener = Arc::new(Mutex::new(Some(listener)));

    // Add a socket runner subprogram. We don't use the address for anything, ie we accept all connections here
    socket_listener_subprogram(context, 
        move ||  {
            let listener        = Arc::clone(&listener);
            let our_listener    = listener.lock().unwrap().take().unwrap();

            async move {
                let connection = our_listener.accept().await
                    .map(|(socket, _addr)| socket.into_split())
                    .map_err(|tokio_err| tokio_err.to_connection_error());

                *listener.lock().unwrap() = Some(our_listener);

                connection
            }
        },
        create_input_messages,
        create_output_messages).await;
}

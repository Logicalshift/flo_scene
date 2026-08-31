use super::socket::*;

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

use tokio::io::{stdin, stdout, AsyncRead, AsyncWrite};

use std::sync::*;

///
/// Starts a sub-program that accepts a single connection on the standard input.
///
/// The program will wait for subscribers (the `Subscribe` message) to the `SocketMessage<TInputStream::Item, TOutputStream::Item>`
/// message. Typically, there's only one subscriber but in the event multiple are connected, they are informed of connections in
/// a round-robin fashion.
///
pub fn start_stdio<TInputStream, TOutputMessage>(
        scene:                  &Scene, 
        program_id:             SubProgramId, 
        create_input_messages:  impl 'static + Send + Sync + Fn(BoxStream<'static, Vec<u8>>) -> TInputStream,
        create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, TOutputMessage>) -> BoxStream<'static, Vec<u8>>
    ) -> Result<(), ConnectionError> 
where
    TInputStream:   'static + Send + Stream,
    TOutputMessage: 'static + Send,
{
    scene.add_subprogram(program_id, 
        move |_input: InputStream<()>, context| {
            context.i_am("Stdio socket connection");

            oneshot_stream_connection_subprogram(context, move || async move { Ok((stdin(), stdout())) }, create_input_messages, create_output_messages)
        },
        0);

    // Success
    Ok(())
}

///
/// Implements a subprogram that receives input from a oneshot generator (such as stdin/stdout) and connects it immediately
///
pub async fn oneshot_stream_connection_subprogram<TInputStream, TOutputMessage, TFutureStream, TReadStream, TWriteStream>(
        context:                SceneContext,
        create_streams:         impl 'static + Send + FnOnce() -> TFutureStream,
        create_input_messages:  impl 'static + Send + Sync + Fn(BoxStream<'static, Vec<u8>>) -> TInputStream,
        create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, TOutputMessage>) -> BoxStream<'static, Vec<u8>>
    )
where
    TInputStream:   'static + Send + Stream,
    TOutputMessage: 'static + Send,
    TFutureStream:  Send + Future<Output=Result<(TReadStream, TWriteStream), ConnectionError>>,
    TReadStream:    'static + Send + AsyncRead,
    TWriteStream:   'static + Send + AsyncWrite,
{
    let waiting = Arc::new(Mutex::new(Some(create_streams)));

    // Add a socket runner subprogram. We don't use the address for anything, ie we accept all connections here
    socket_listener_subprogram(context, 
        move || {
            let waiting = waiting.clone();

            async move {
                let create_streams = waiting.lock().unwrap().take();

                if let Some(create_streams) = create_streams {
                    create_streams().await
                } else {
                    // Otherwise, just loop forever
                    loop {
                        future::pending::<()>().await;
                    }
                }
            }
        },
        create_input_messages,
        create_output_messages).await;
}

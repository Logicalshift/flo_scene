use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::{Stream, Future};
use futures::stream;
use futures::stream::{BoxStream, StreamExt};
use futures::{pin_mut};


use tokio::io::*;

use std::result::{Result};
use std::sync::*;

///
/// Represents an incoming socket connection. When a socket is connected, we retrieve an input stream, and need to respond with an output stream.
///
/// The socket is closed when the output stream is closed, or when the connection itself is dropped.
///
pub struct SocketConnection<TInputMessage, TOutputMessage> {
    /// The context that this socket is being created in
    context: SceneContext,

    /// The input stream for this socket
    input_stream: Option<BoxStream<'static, TInputMessage>>,

    /// Sends the output of a stream as the response to a socket (set to None once the socket is created)
    create_output_stream: Option<Box<dyn Send + FnOnce(&SceneContext, BoxStream<'static, TOutputMessage>) -> ()>>,
}

///
/// Event message sent from a program that represents a socket. Socket programs represent points where two-way connections
/// can be made to this program.
///
/// Socket connections are usually made from outside the active program, and most typically representing a UNIX socket or an 
/// internet socket. This API covers any such connection with similar semantics though, and is not limited to these types of
/// socket. Some parsing is usually also applied to the raw stream, for example to read the input of a socket as a series
/// of JSON messages.
///
pub enum SocketMessage<TInputMessage, TOutputMessage> {
    ///
    /// Indicates that a new connection has been made to this socket. 
    ///
    Connection(SocketConnection<TInputMessage, TOutputMessage>)
}

impl<TInputMessage, TOutputMessage> SceneMessage for SocketMessage<TInputMessage, TOutputMessage> { }

impl<TInputMessage, TOutputMessage> SocketConnection<TInputMessage, TOutputMessage> 
where
    TInputMessage:  'static,
    TOutputMessage: 'static,
{
    ///
    /// Creates a new socket connection
    ///
    pub fn new(context: &SceneContext, input: impl 'static + Send + Stream<Item=TInputMessage>, send_output: impl 'static + Send + FnOnce(&SceneContext, BoxStream<'static, TOutputMessage>) -> ()) -> Self {
        SocketConnection {
            context:                context.clone(),
            input_stream:           Some(input.boxed()),
            create_output_stream:   Some(Box::new(send_output)),
        }
    }

    ///
    /// Sets the stream that will send the resulting output to the socket, and returns the input stream that can be used to read incoming data
    ///
    pub fn connect(mut self, output_stream: impl 'static + Send + Stream<Item=TOutputMessage>) -> BoxStream<'static, TInputMessage> {
        // Take the components out of the structure
        let create_output_stream    = self.create_output_stream.take().unwrap();
        let input_stream            = self.input_stream.take().unwrap();

        // Create the output stream
        (create_output_stream)(&self.context, output_stream.boxed());

        // Return the input stream
        input_stream
    }
}

///
/// Runs a socket listener suprogram. This accepts 'Subscribe' messages from subprograms that wish to receive connections (subscription messages are sent in a round-robin fashion),
/// and calls the 'accept_message' function to receive incoming connections
///
pub async fn socket_listener_subprogram<TFutureStream, TReadStream, TWriteStream, TInputStream, TOutputStream>(subscribe: impl 'static + Send + Stream<Item=(SubProgramId, Subscribe)>, context: SceneContext, 
    accept_connection: impl 'static + Send + Fn() -> TFutureStream,
    create_input_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, u8>) -> TInputStream,
    create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, u8>) -> TOutputStream)
where
    TFutureStream:  Send + Future<Output=Result<(TReadStream, TWriteStream), ConnectionError>>,
    TReadStream:    'static + Send + AsyncRead,
    TWriteStream:   'static + Send + AsyncWrite,
    TInputStream:   'static + Send + Stream,
    TOutputStream:  'static + Send + Stream,
{
    // Wrap accept_connection in a desync
    let accept_connection = Arc::new(accept_connection);

    // Combine the subscription and the acceptance streams
    enum OurMessage<TSocketStream> {
        Subscribe(SubProgramId),
        NewConnection(TSocketStream),
    }

    let subscribe       = subscribe.map(|(sender, _)| OurMessage::Subscribe(sender));
    let accept_messages = stream::unfold(0, move |_| {
        let accept_connection = Arc::clone(&accept_connection);

        async move {
            // Fetch the next connection if there is one
            let next_connection = accept_connection().await;

            // Continue until we get an error
            match next_connection {
                Ok(next_connection) => Some((OurMessage::NewConnection(next_connection), 0)),
                _                   => None,
            }
        }
    });

    pin_mut!(subscribe);
    pin_mut!(accept_messages);
    let mut input = stream::select(subscribe, accept_messages);

    // Run the socket listener
    let mut subscribers         = vec![];
    // let mut waiting_connections = vec![];

    while let Some(next_event) = input.next().await {
        match next_event {
            OurMessage::Subscribe(program_id) => {
                // Add this program to the list of subscribers for our message type (any one subscriber can only be added once)
                subscribers.retain(|prog| prog != &program_id);
                subscribers.push(program_id);
            }

            OurMessage::NewConnection(socket_stream) => {
                // Create the socket connection


                // Try to send the connection to the first subscriber that can receive the message
            }
        }
    }
}

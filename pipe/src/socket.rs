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
/// Creates a stream that reads blocks of data from an AsyncRead
///
fn create_reader_stream(reader: impl 'static + Send + AsyncRead) -> impl Stream<Item=Vec<u8>> {
    let reader = Box::pin(reader);
    let reader = Arc::new(Mutex::new(Some(reader)));

    stream::unfold([0u8; 64], move |mut buf| {
        // Create a copy of the reader to use in the async block
        let reader = Arc::clone(&reader);

        async move {
            // Take the reader from the mutex and read the next value
            let mut borrowed_reader     = reader.lock().unwrap().take().unwrap();
            let next_read               = borrowed_reader.read(&mut buf).await;
            *(reader.lock().unwrap())   = Some(borrowed_reader);

            // Return the next set of bytes we read from the input stream if available (or close the stream if there's an error or the end of stream is reached)
            match next_read {
                Ok(0)           => None,
                Ok(num_read)    => Some((buf[0..num_read].into(), buf)),
                Err(_)          => None,
            }
        }
    })
}

///
/// Runs a socket listener suprogram. This accepts 'Subscribe' messages from subprograms that wish to receive connections (subscription messages are sent in a round-robin fashion),
/// and calls the 'accept_message' function to receive incoming connections
///
pub async fn socket_listener_subprogram<TFutureStream, TReadStream, TWriteStream, TInputStream, TOutputMessage>(
    subscribe:              impl 'static + Send + Stream<Item=(SubProgramId, Subscribe)>, 
    context:                SceneContext, 
    accept_connection:      impl 'static + Send + Fn() -> TFutureStream,
    create_input_messages:  impl 'static + Send + Sync + Fn(BoxStream<'static, Vec<u8>>) -> TInputStream,
    create_output_messages: impl 'static + Send + Sync + Fn(BoxStream<'static, TOutputMessage>) -> BoxStream<'static, Vec<u8>>)
where
    TFutureStream:  Send + Future<Output=Result<(TReadStream, TWriteStream), ConnectionError>>,
    TReadStream:    'static + Send + AsyncRead,
    TWriteStream:   'static + Send + AsyncWrite,
    TInputStream:   'static + Send + Stream,
    TOutputMessage: 'static + Send ,
{
    // Wrap functions that get shared in a reference
    let accept_connection       = Arc::new(accept_connection);
    let create_output_messages  = Arc::new(create_output_messages);

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
            OurMessage::NewConnection((async_reader, async_writer)) => {
                // Create the socket connection from the reader
                let reader_stream = create_reader_stream(async_reader);
                let reader_stream = create_input_messages(reader_stream.boxed());

                let create_output_messages  = Arc::clone(&create_output_messages);
                let socket_connection       = SocketConnection::new(&context, reader_stream, move |context, output_stream| {
                    // Create a stream that converts to bytes
                    let mut output_byte_stream = create_output_messages(output_stream);

                    // Future to write the bytes
                    let async_writer = Box::pin(async_writer);
                    let byte_writer  = async move {
                        let mut async_writer = async_writer;
                        while let Some(bytes) = output_byte_stream.next().await {
                            async_writer.write(&bytes).await.unwrap();
                        }
                    };

                    // Ask the scene to create a subprogram that writes the output (won't work if the main 'scene' program isn't running)
                    let output_program = SubProgramId::new();
                    // let output_program = SceneControl::start_program(output_program, move |_: InputStream<()>, context| byte_writer, 0);

                    //let mut control = context.send(()).unwrap();
                    //control.send_immediate(output_program);
                });

                // Try to send the connection to the first subscriber that can receive the message
            }

            OurMessage::Subscribe(program_id) => {
                // Add this program to the list of subscribers for our message type (any one subscriber can only be added once)
                subscribers.retain(|prog| prog != &program_id);
                subscribers.push(program_id);

                // TODO: If there are any waiting connections, send them all to this program
            }
        }
    }
}

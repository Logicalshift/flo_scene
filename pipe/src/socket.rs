use flo_scene::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

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
